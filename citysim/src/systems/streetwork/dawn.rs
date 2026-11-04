use super::*;

/// § 3, at `corner_hour`: the day's posts filled from the corner's queue
/// (bodies standing there; off screen, the broke living near on
/// `p_corner`), each post from the nearest corner's queue: the
/// employer's foreman's known contacts first, then the strongest
/// (`fighting`), then id. A pick is hired into the `day` row with
/// `Job.until` two hours past the shift's end (`DawnPick` per corner).
pub(super) fn dawn_pick(world: &mut World) {
    let Some(role) = day_role(world) else { return };
    // Round 3 S5 (OPEN-217): no shape-up on the rest day: the post's shift
    // is a workday's (`routine::workday_of`), so a hand picked then held a
    // post with no shift and wandered all day (`day_labourer_75212 d27`).
    if world.config.verbs.is_some() && !crate::exec::routine::is_workday(world.day() as i64) {
        return;
    }
    let cfg = world.config.jobs2.clone();
    let corners = corners(world);
    if corners.is_empty() {
        return;
    }
    let wage = world.config.wage(role);
    let posts = day_posts(world, wage);
    ownership::jobs2_row(world).day_posts_offered += posts.len() as u32;
    world.jobs2.last_posts = posts.len() as u32;
    // The queues, one per corner.
    let day = world.day();
    let mut queues: Vec<Vec<EntityId>> = vec![Vec::new(); corners.len()];
    // scan-ok: daily: the dawn pick's queue
    for a in world.citizens() {
        if !world.has::<Brain>(a) || !corner_ok(world, a) {
            continue;
        }
        let body = world.comp::<Brain>(a).is_some_and(|b| b.lod != Lod::Statistical);
        let at = if body {
            world
                .comp::<Position>(a)
                .filter(|p| p.building.is_none())
                .and_then(|p| corner_near(&corners, p.tile, cfg.corner_tiles))
        } else {
            let from =
                crate::systems::contracts::home_door(world, a).or_else(|| world.comp::<Position>(a).map(|p| p.tile));
            // The roll first: the travel fallback is the dear part.
            let rolled = hash(world, PURPOSE_CORNER, u64::from(a.index), day) < cfg.p_corner;
            from.filter(|_| rolled).and_then(|t| {
                corner_near(&corners, t, cfg.corner_stat_reach).or_else(|| corner_by_travel(world, &corners, t))
            })
        };
        if let Some(i) = at {
            queues[i].push(a);
        }
    }
    let queued: u32 = queues.iter().map(|q| q.len() as u32).sum();
    ownership::jobs2_row(world).corner_queue += queued;
    let mut picked = vec![0u32; corners.len()];
    // Round 4 T4 (OPEN-217): each Yard's morning (its corner, its foreman,
    // whom he picked), told after the pick.
    let gate = world.config.verbs.as_ref().is_some_and(|v| v.shape_up_gate);
    let mut mornings: Vec<Morning> = Vec::new();
    let shift_end = world.config.world.shift_day.iter().map(|&(_, e)| e).max().unwrap_or(1080);
    let until = world.tick - Tick::from(world.tick_of_day()) + Tick::from(shift_end) + 2 * TICKS_PER_HOUR;
    let mut posted: Vec<EntityId> = posts.clone();
    posted.dedup();
    for b in posts {
        let Some(door) = world.comp::<Building>(b).map(|x| x.door) else { continue };
        let Some(i) = corner_near(&corners, door, u32::MAX) else { continue };
        let foreman = crate::systems::industry::foreman(world, b, None);
        // Lives at work phase 6 (§ 4.1): the corner's band (every one off).
        let band = crate::systems::careers::hire_band(world, role);
        let order = |&x: &EntityId, &y: &EntityId| {
            let known = |a: EntityId| foreman.is_some_and(|f| world.edge(a, f).is_some());
            let f =
                |a: EntityId| world.comp::<Skills>(a).map_or(0.0, |s| crate::systems::combat::skill_power(world, s));
            known(x).cmp(&known(y)).then(f(x).total_cmp(&f(y))).then(y.cmp(&x))
        };
        let admitted = queues[i]
            .iter()
            .copied()
            .filter(|&a| !world.has::<Job>(a) && crate::systems::careers::admits(world, a, band));
        let best = match world.config.distance.as_ref() {
            // The island pass phase 1 (OPEN-163): in the foreman's order, a
            // labourer past `[distance] job_cap_ticks` from the post (from
            // where he stands, else his Home door) is passed over while one
            // within it is left, else the nearest.
            Some(cfg) => {
                let mut ranked: Vec<EntityId> = admitted.collect();
                ranked.sort_by(|x, y| order(y, x));
                crate::systems::distance::pick(
                    ranked.iter().enumerate().map(|(r, &a)| {
                        let from = world
                            .comp::<crate::components::Position>(a)
                            .map(|p| p.tile)
                            .or_else(|| crate::systems::distance::set_out(world, a));
                        let t = from.map_or(Tick::MAX, |t| crate::systems::distance::travel(world, t, door));
                        (t, r as i64, (r, a))
                    }),
                    cfg.job_cap_ticks,
                )
                .map(|(_, a)| a)
            }
            None => admitted.max_by(order),
        };
        let Some(a) = best else { continue };
        // Pressure pass round 3, S1 (OPEN-221): the nearest left is no hire
        // past `[distance.choice] hire_max_ticks` of the post.
        if let Some(c) = crate::systems::distance::choice(world) {
            let from = world
                .comp::<crate::components::Position>(a)
                .map(|p| p.tile)
                .or_else(|| crate::systems::distance::set_out(world, a));
            // The structural trip, as `demography::past_hire_max`.
            if from.is_some_and(|t| crate::systems::distance::travel(world, t, door) > c.hire_max_ticks) {
                ownership::jobs2_row(world).choice_hire_far += 1;
                continue;
            }
        }
        queues[i].retain(|&x| x != a);
        // Round 4 T4: the pick's Hire is the hand's; the foreman's diary has
        // the morning's one line (174 Hire lines a week).
        let yard = gate
            && world
                .comp::<Building>(b)
                .is_some_and(|bd| matches!(bd.kind, BuildingKind::ConstructionYard | BuildingKind::FreightYard));
        crate::systems::demography::hire_with(world, a, b, role, !yard);
        if yard {
            match mornings.iter_mut().find(|m| m.0 == b) {
                Some(m) => m.3.push(a),
                None => mornings.push((b, i, foreman, vec![a])),
            }
        }
        // Round 4 T5 (OPEN-217): the hand works the day he was picked for,
        // from `[search] day_start_min` after the pick to the day shift's
        // end (V12's rested three hours at the Yard for a 09:00 shift).
        let start = crate::systems::search::cfg(world)
            .map(|c| (cfg.corner_hour + c.day_start_min).min(shift_end.saturating_sub(1)));
        if let Some(j) = world.comp_mut::<Job>(a) {
            j.until = Some(until);
            if let Some(s) = start {
                j.shifts = vec![(s, shift_end)];
            }
        }
        // The pressure pass phase 2: the shape-up is a meeting (both diaries).
        // M18 P5h: a `Served` memory is the service section's (the off city
        // remembers none, `service`'s `test_off_city_writes_nothing`).
        if let Some(f) =
            foreman.filter(|_| crate::systems::customers::cfg(world).is_some() && crate::systems::service::on(world))
        {
            let sal = world.config.service.served_salience;
            world.remember(f, crate::components::MemoryKind::Served, Some(a), sal, 0.1, false);
            world.remember(a, crate::components::MemoryKind::Served, Some(f), sal, 0.2, false);
        }
        picked[i] += 1;
        ownership::jobs2_row(world).day_posts_filled += 1;
    }
    if gate {
        shape_up_told(world, &mornings, &queues, &posted);
    }
    for (i, &(d, _)) in corners.iter().enumerate() {
        let left = queues[i].len();
        if picked[i] == 0 && left == 0 {
            continue;
        }
        let at = world.district_name(d).to_string();
        world.push_event(
            EventKind::DawnPick,
            &[],
            format!("the labour corner in {at}: {} picked at dawn, {left} sent away", picked[i]),
        );
    }
}

/// Round 4 T4: a Yard's morning at the shape-up: the Yard, its corner, its
/// foreman, the hands he picked.
type Morning = (EntityId, usize, Option<EntityId>, Vec<EntityId>);

/// Round 4 T4 (OPEN-217): the shape-up told. One line a Yard that picked
/// (its foreman an actor: at the gate, or "not at the gate" when his body
/// was elsewhere), naming the picked and the corner's sent home; each body
/// sent home its own line and a memory of the foreman who passed him over
/// (the Yard that took the most at his corner).
fn shape_up_told(world: &mut World, mornings: &[Morning], queues: &[Vec<EntityId>], posted: &[EntityId]) {
    for (y, i, foreman, hands) in mornings {
        let left = &queues[*i];
        let names = crate::systems::vice::named(world, hands, hands.len());
        let home = if left.is_empty() {
            "nobody sent home".to_string()
        } else {
            format!("{} sent home ({})", left.len(), crate::systems::vice::named(world, left, left.len()))
        };
        let at = world.name_of(*y);
        let text = match foreman {
            Some(f) if at_gate(world, *f, *y) => {
                if world.comp::<Brain>(*f).is_some_and(|b| b.lod != Lod::Statistical) {
                    ownership::jobs2_row(world).foreman_gate += 1;
                }
                format!("{} picked {} at the gate of {at} at dawn: {names}; {home}", world.name_of(*f), hands.len())
            }
            Some(f) => format!(
                "{at} took {} at dawn off the list, the foreman {} not at the gate: {names}; {home}",
                hands.len(),
                world.name_of(*f)
            ),
            None => format!("{at} took {} at dawn: {names}; {home}", hands.len()),
        };
        let actors: Vec<EntityId> = foreman.iter().copied().chain([*y]).collect();
        world.push_event(EventKind::DawnPick, &actors, text);
    }
    // A foreman's body at the gate of a Yard that took nobody: his morning
    // too (he walked there for it).
    let mut yards: Vec<EntityId> = [BuildingKind::ConstructionYard, BuildingKind::FreightYard]
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter(|y| !mornings.iter().any(|m| m.0 == *y))
        .collect();
    yards.sort_unstable();
    for y in yards {
        let Some(f) = crate::systems::industry::foreman(world, y, None) else { continue };
        if world.comp::<Brain>(f).is_none_or(|b| b.lod == Lod::Statistical) || !at_gate(world, f, y) {
            continue;
        }
        let why = if posted.contains(&y) { "nobody at the gate he would take" } else { "no post the owner pays today" };
        let text =
            format!("{} opened the gate of {} at dawn and took nobody: {why}", world.name_of(f), world.name_of(y));
        world.push_event(EventKind::DawnPick, &[f, y], text);
        ownership::jobs2_row(world).foreman_gate += 1;
    }
    // The bodies sent home: the corner's busiest Yard passed them over.
    let sal = world.config.service.served_salience;
    for (i, q) in queues.iter().enumerate() {
        let Some((y, f)) = mornings
            .iter()
            .filter(|m| m.1 == i)
            .max_by_key(|m| (m.3.len(), std::cmp::Reverse(m.0)))
            .map(|m| (m.0, m.2))
        else {
            continue;
        };
        for &a in q {
            if world.comp::<Brain>(a).is_none_or(|b| b.lod == Lod::Statistical) {
                continue;
            }
            let by = f.map(|f| format!(" ({} picked others)", world.name_of(f))).unwrap_or_default();
            let text =
                format!("{} stood at the gate of {} at dawn and was sent home{by}", world.name_of(a), world.name_of(y));
            // His line, not the foreman's (the foreman's is the Yard's one).
            world.push_event(EventKind::DawnPick, &[a, y], text);
            // Round 4 T5 (OPEN-217): he goes on looking today.
            crate::systems::search::note_sent_home(world, a);
            if let Some(f) = f.filter(|_| crate::systems::service::on(world)) {
                world.remember(a, crate::components::MemoryKind::Served, Some(f), sal, -0.2, false);
            }
            ownership::jobs2_row(world).shape_up_home += 1;
        }
    }
}
