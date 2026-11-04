use super::*;

/// The skill a row trains.
pub(crate) fn train(world: &mut World, server: EntityId, skill: TradeSkill, gain: f32) {
    if gain <= 0.0 {
        return;
    }
    let Some(s) = world.comp_mut::<crate::components::Skills>(server) else { return };
    let slot = match skill {
        TradeSkill::None => return,
        TradeSkill::Farming => &mut s.farming,
        TradeSkill::Fighting => &mut s.melee,
        TradeSkill::Persuasion => &mut s.persuasion,
        TradeSkill::Knowledge => &mut s.knowledge,
        TradeSkill::Hacking => &mut s.hacking,
    };
    *slot = (*slot + gain).min(1.0);
}

/// One encounter (§ 1.2's five steps) at a place.
pub fn serve(world: &mut World, server: EntityId, served: EntityId, place: EntityId, kind: ServeKind, coins: i64) {
    // The pre-view pass phase 3 (OPEN-145): the device rows, a venue's
    // customers by day and in the 21:00 hour (the Statistical evening).
    if kind == ServeKind::Visit && crate::systems::shift::counter_on(world) {
        let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
        let row = ownership::jobs2_row(world);
        row.served_day += u32::from((6..18).contains(&hour));
        row.served_21h += u32::from(hour == 21);
    }
    let d = world.district_of_building(place);
    encounter(world, server, served, Some(place), d, kind, coins);
    // The pressure pass R6a (OPEN-217): a Brothel visit is the madam's booking.
    if kind == ServeKind::Visit {
        crate::systems::vice::booked(world, server, served, place, coins);
        // Round 3 S5 (OPEN-217): a rough client at a kept door, the bouncer's.
        crate::systems::customers::rough_at_door(world, server, served, place);
    }
}

/// Lives at work phase 4 (§ 2.3): the encounter, at a place or (a street
/// pitch, a corner, a rickshaw's stand: a tile, not a Building) in a
/// district alone; the event names the district when there is no place.
/// `serve` is the placed form. A server with no trade row (the pimp, the
/// dealer) is read by its gang post's row (`row_of_agent`).
pub fn encounter(
    world: &mut World,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let Some(row) = row_of_agent(world, server).cloned() else { return };
    encounter_row(world, row, server, served, place, district, kind, coins);
}

/// Lives at work phase 4b (§ 2.3): the encounter of a gang post with no
/// trade row, read by its `[[serves]]` key (`"pimp"`, `"dealer"`): a gang
/// member holding a Job meets his collection or his buyer as the post, not
/// as his trade. Nothing when the key has no row.
#[allow(clippy::too_many_arguments)]
pub fn encounter_post(
    world: &mut World,
    post: &str,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let Some(row) = world.config.serves.iter().find(|r| r.key == post).cloned() else { return };
    encounter_row(world, row, server, served, place, district, kind, coins);
}

#[allow(clippy::too_many_arguments)]
fn encounter_row(
    world: &mut World,
    row: ServesCfg,
    server: EntityId,
    served: EntityId,
    place: Option<EntityId>,
    district: crate::components::DistrictId,
    kind: ServeKind,
    coins: i64,
) {
    let kind_label = kind.label();
    let place_kind = place.and_then(|b| world.comp::<Building>(b)).map(|b| b.kind);
    // 1. The arm: the counters and the re-routed coins.
    {
        let r = ownership::jobs2_row(world);
        r.served[kind as usize] += 1;
        if kind == ServeKind::Visit && place_kind == Some(BuildingKind::NoodleBar) {
            r.cooked += 1;
        }
    }
    let salience = world.config.service.served_salience;
    // The tip (§ 2.1): to the server when its trade row is tipped, else to
    // the tipped worker on duty the hash names (a Club's Host serves the
    // visit, the dancer is tipped: the flow main paid, re-routed, D4).
    let tipped =
        if row_tipped(world, &row.key) { Some(server) } else { place.and_then(|b| tipped_pick(world, b, served)) };
    if kind == ServeKind::Visit && coins > 0 {
        if let Some(t) = tipped.filter(|&t| t != served) {
            let tip = ((coins as f32 * world.config.vice.tip_share).round() as i64).max(1);
            let paid = ownership::pay(world, Some(served), Some(t), tip, Flow::Tip);
            // Round 4 T4 (OPEN-217): the tip on a serving body's night.
            if let Some(place) = place {
                crate::systems::vice::night_tip(world, t, served, place, paid);
            }
            // § 2.1, 8d (OPEN-123): a tip at her own house is noted on the
            // worker's shift; the madam's cut of the total settles once at
            // `shift_end` (per tip, 0.3 of a 1 c tip rounded to nothing).
            if let (true, Some(BuildingKind::Brothel), Some(place)) = (paid > 0, place_kind, place) {
                if let Some(j) = world.comp_mut::<Job>(t).filter(|j| j.employer == Some(place)) {
                    j.shift_tips += paid;
                }
            }
        }
    }
    // Lives at work phase 3 (§ 2.2): the civic arms.
    let mut outcome = Outcome::Done;
    match (kind, place) {
        (ServeKind::Pupil, Some(_)) => taught(world, server, served, row.valence),
        (ServeKind::Tenant, Some(p)) => outcome = tenant(world, server, served, p),
        _ => {}
    }
    // 2. Memories, bodies only (`remember` drops a Statistical agent's and a chip's).
    // The pressure pass R4 (OPEN-219): a super's call at a door with nothing
    // in it (the row's zero valence, no notice: `tenant` wrote the notice's
    // own) is no memory either side (V10: ~40 "called on" lines a round).
    let blank_call = kind == ServeKind::Tenant && row.valence == 0.0;
    // The pressure pass S6 (OPEN-224): a pupil is no memory of the
    // teacher's (a child has none of its own; a roster of 30 a morning
    // buried the teacher's memory, and the diary read the pupil as "a
    // pupil's parent"): the day's class is one event (`civic_hour`).
    let pupil = kind == ServeKind::Pupil && world.has::<Child>(served);
    // Round 4 T1 (OPEN-231): nor a Camp child at the warden's count (the
    // headcount is one `Custody` line, `camp::headcount`).
    let counted = kind == ServeKind::Inmate && world.has::<Child>(served);
    if !blank_call && !pupil && !counted {
        // M18 P4 (OPEN-149, nothing in camera range is Statistical): a party
        // to an encounter with a body the human drives, or one the camera
        // ring holds, is a body here, so its memory of the meeting is kept
        // (a Fixer's Statistical desk hand forgot the player's take, P3).
        for (x, y) in [(server, served), (served, server)] {
            let seen = crate::systems::player::drives(world, y) || world.ring.holds(y);
            if seen
                && world
                    .comp::<crate::components::Brain>(x)
                    .is_some_and(|b| b.lod == crate::components::Lod::Statistical)
            {
                crate::systems::lod::set_lod_at(world, x, crate::components::Lod::Full, place);
            }
        }
        world.remember(server, MemoryKind::Served, Some(served), salience, row.valence, false);
        world.remember(served, MemoryKind::Served, Some(server), salience, row.valence, false);
    }
    // 3. The regulars ledger and the edge.
    let regular = note_regular(world, server, served, &row);
    // Round 4 T4 (OPEN-217): the served on a serving body's night (the
    // regulars: the ledger's count at `regular_after` or past it).
    if let (ServeKind::Visit, Some(p)) = (kind, place) {
        let reg = row.regular_after > 0
            && world
                .regulars
                .get(&server)
                .and_then(|l| l.iter().find(|r| r.who == served))
                .is_some_and(|r| r.visits >= row.regular_after);
        crate::systems::vice::night_served(world, server, served, p, reg);
    }
    // 4. The skill.
    train(world, server, row.skill, row.skill_gain);
    // 5. The event.
    if regular {
        outcome = Outcome::Regular;
    }
    if row.event_on.contains(&outcome.cfg()) {
        // The pressure pass R4: a notice on the round is folded into its line.
        if outcome == Outcome::Notice {
            if let Some(f) = world.notice_fold.get_mut(&server) {
                f.push(served);
                return;
            }
        }
        let (who, whom) = (world.name_of(server), world.name_of(served));
        let at = match place {
            Some(p) => world.name_of(p),
            None => format!("a street in {}", world.district_name(district)),
        };
        let (kind, text) = match outcome {
            Outcome::Regular => (EventKind::Served, format!("{whom} is now a regular of {who} at {at}")),
            Outcome::Notice => (EventKind::Notice, format!("{who} served notice on {whom} at {at}")),
            _ => (EventKind::Served, format!("{who} served {whom} ({kind_label}) at {at}")),
        };
        let actors: Vec<EntityId> = [Some(server), Some(served), place].into_iter().flatten().collect();
        world.push_event(kind, &actors, text);
    }
}

/// The ledger: `visits + 1` within `regular_days`, else a fresh count; at
/// exactly `regular_after` with no edge, an Acquaintance at a hashed
/// affinity (never the world stream); with an edge, `interacted`.
fn note_regular(world: &mut World, server: EntityId, served: EntityId, row: &ServesCfg) -> bool {
    let day = world.day();
    let today = u16::try_from(day).unwrap_or(u16::MAX);
    let window = u16::from(row.regular_days);
    let list = world.regulars.entry(server).or_default();
    let visits = match list.iter_mut().find(|r| r.who == served) {
        Some(r) => {
            r.visits = if today.saturating_sub(r.last_day) > window { 1 } else { r.visits.saturating_add(1) };
            r.last_day = today;
            r.visits
        }
        None => {
            if list.len() >= REGULARS_CAP {
                // The oldest falls off (ties: the first).
                let (i, _) = list.iter().enumerate().min_by_key(|(i, r)| (r.last_day, *i)).expect("non-empty");
                list.remove(i);
            }
            list.push(Regular { who: served, visits: 1, last_day: today });
            1
        }
    };
    if world.edge(server, served).is_some() {
        crate::systems::social::interacted(world, server, served);
        return false;
    }
    if row.regular_after > 0 && visits == row.regular_after {
        // The affinity of a regular's acquaintance: a hashed `0.1..0.4`.
        let u = hash(world, PURPOSE_REGULAR, key(server), key(served));
        crate::systems::social::first_meeting(world, server, served, 0.1 + 0.3 * u);
        ownership::jobs2_row(world).regulars_made += 1;
        return true;
    }
    false
}

/// The door staff of `b` on duty (a fighting trade on shift, present).
fn door_staff(world: &World, b: EntityId) -> Vec<EntityId> {
    crate::systems::trades::door_staff(world, b).into_iter().filter(|&s| present(world, s, b)).collect()
}

/// Round 4 T4 (OPEN-217; `[verbs] door_body_first`): the bouncer at `b`'s
/// door who meets `id` now: of the present door staff (`exclude` left out),
/// a body first (the one standing the door tile), the hash of the visitor
/// and the hour among them; else among all. V12's 100k bouncers kept 28
/// nights with no door line: an off-screen colleague, present by the clock
/// on any day, took every let-in as the lowest id and half the refusals.
/// Without the key: the hash among all.
pub fn door_man(world: &World, b: EntityId, id: EntityId, exclude: &[EntityId]) -> Option<EntityId> {
    let staff: Vec<EntityId> = door_staff(world, b).into_iter().filter(|s| !exclude.contains(s)).collect();
    let first = world.config.verbs.as_ref().is_some_and(|v| v.door_body_first);
    let bodies: Vec<EntityId> = if first {
        staff.iter().copied().filter(|&s| world.comp::<Brain>(s).is_some_and(|br| br.lod != Lod::Statistical)).collect()
    } else {
        Vec::new()
    };
    let pool = if bodies.is_empty() { staff } else { bodies };
    if pool.is_empty() {
        return None;
    }
    let u = hash(world, PURPOSE_DOOR, key(id), world.tick / TICKS_PER_HOUR);
    Some(pool[((u * pool.len() as f32) as usize).min(pool.len() - 1)])
}

/// § 2.1 (bouncer): `id` was turned away at `b`'s door (the exec's failed
/// start of a venue step inside it): `Refused`, an event when the bouncer's
/// row names it, a memory each side, the counter. Nothing off.
pub fn refused(world: &mut World, id: EntityId, b: EntityId) {
    if !on(world) {
        return;
    }
    ownership::jobs2_row(world).door_refusals += 1;
    // The pre-view pass phase 3 (OPEN-121): turned away, the patron's next
    // pick passes this door over for `[exec] refused_ticks` (`leisure::door_seen`).
    if crate::systems::shift::counter_on(world) {
        let tick = world.tick;
        if let Some(br) = world.comp_mut::<crate::components::Brain>(id) {
            br.refused = Some((b, tick));
        }
    }
    // Round 4 T7 (OPEN-233): back at a door that turned him away inside
    // `[verbs] door_return_ticks` is a return, and the door's answer changes:
    // he is barred from the house `door_return_ban_days` (`vice::barred`,
    // which `leisure::door_seen` passes over) and the scene he makes of it
    // is likelier. T4's 100k door turned one armed man away nine times in
    // four nights, the pass-over (`[exec] refused_ticks`, one door a face)
    // all that held him off.
    let (ret_ticks, ban_days, swing) = world
        .config
        .verbs
        .as_ref()
        .map_or((0, 0, 1.0), |v| (v.door_return_ticks, v.door_return_ban_days, v.door_return_swing_mult));
    let back = ret_ticks > 0 && crate::systems::vice::turned_again(world, id, b, ret_ticks);
    // Round 4 T4: the bouncer standing the door (a body first).
    let Some(bouncer) = door_man(world, b, id, &[]) else { return };
    let Some(row) = row_of_agent(world, bouncer).cloned() else { return };
    let salience = world.config.service.served_salience;
    let (vb, vr) = (world.config.service.tune.refused_bouncer, world.config.service.tune.refused);
    world.remember(bouncer, MemoryKind::Served, Some(id), salience, vb, false);
    world.remember(id, MemoryKind::Served, Some(bouncer), salience, vr, false);
    // The pressure pass phase 2 (OPEN-176): the cause named, and the scene
    // a drunk or a brawler makes of it.
    let why = crate::systems::customers::cause(world, id, b);
    if why == Some("barred") {
        ownership::jobs2_row(world).door_barred += 1;
    }
    if why == Some("under age") {
        ownership::jobs2_row(world).door_under_age += 1;
    }
    if why == Some("drunk") {
        ownership::jobs2_row(world).door_drunk += 1;
    }
    // Round 7 W6 (OPEN-217): the off-screen visitor's refusals by cause (V15:
    // "broke" the only one a 100k door ever gave).
    if world.comp::<crate::components::Brain>(id).is_some_and(|b| b.lod == crate::components::Lod::Statistical) {
        let row = ownership::jobs2_row(world);
        match why {
            Some("broke") => row.door_stat_broke += 1,
            Some("barred") => row.door_stat_barred += 1,
            Some("drunk" | "high" | "a brawler" | "carrying" | "trouble") => row.door_stat_trouble += 1,
            Some("a rival's colours") => row.door_stat_colours += 1,
            _ => row.door_stat_other += 1,
        }
    }
    if world.config.verbs.is_some() {
        let c = why.unwrap_or("no cause").to_string();
        crate::systems::vice::door_note(world, bouncer, b, |n| {
            n.refused += 1;
            match n.causes.iter_mut().find(|(k, _)| *k == c) {
                Some((_, k)) => *k += 1,
                None => n.causes.push((c, 1)),
            }
        });
    }
    // ... the bar, a door's own (`vice::shut_out`; R6a's `bar` holds one a face).
    let mut barred_till: Option<u64> = None;
    if back && why != Some("barred") {
        let until = world.day() + ban_days;
        crate::systems::vice::shut_out(world, id, b, until);
        barred_till = Some(until);
    }
    if row.event_on.contains(&ServeOutcome::Refused) {
        let because = why.map(|w| format!(" ({w})")).unwrap_or_default();
        let again = if back { " again" } else { "" };
        // Round 6 V4 (OPEN-239): the last day barred (`vice::barred` is
        // `day < until`; "barred till d26" read as d26 barred, and the d26
        // return as the bar ignored).
        let bar = barred_till.map_or(String::new(), |d| format!(": barred through d{}", d.saturating_sub(1)));
        let text = format!(
            "{} turned {} away{again} at the door of {}{because}{bar}",
            world.name_of(bouncer),
            world.name_of(id),
            world.name_of(b)
        );
        world.push_event(EventKind::Served, &[bouncer, id, b], text);
    }
    let mult = if back { swing } else { 1.0 };
    if let Some(line) = why.and_then(|w| crate::systems::customers::scene(world, id, bouncer, b, w, mult)) {
        world.push_event(EventKind::Brawl, &[id, bouncer, b], line);
    }
}

/// § 2.1 (bouncer): an Assault raised inside a kept venue on a door shift
/// is `Ejected` on `p_eject × (bouncer fighting − attacker fighting)`: the
/// attacker's plan aborted and the attacker put at the door. Returns
/// whether it was thrown out. Nothing off.
pub fn maybe_eject(world: &mut World, attacker: EntityId) -> bool {
    // Round 4 T4 (the under-18 rule): nobody under 18 is thrown out by force
    // (`ages::is_protected`; the door turns him away "under age" first).
    if !on(world) || crate::systems::ages::is_protected(world, attacker) {
        return false;
    }
    let Some(b) = world.comp::<Position>(attacker).and_then(|p| p.building) else { return false };
    let staff = door_staff(world, b);
    let Some((bouncer, strength)) = staff
        .iter()
        .map(|&s| (s, crate::systems::combat::power(world, s)))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
    else {
        return false;
    };
    let edge = (strength - crate::systems::combat::power(world, attacker)).max(0.0);
    let p = world.config.service.p_eject * edge;
    if hash(world, PURPOSE_EJECT, key(attacker), world.tick) >= p {
        return false;
    }
    world.abort_plan(attacker);
    world.leave_building(attacker);
    ownership::jobs2_row(world).ejections += 1;
    if world.config.verbs.is_some() {
        crate::systems::vice::door_note(world, bouncer, b, |n| n.thrown += 1);
    }
    let salience = world.config.service.served_salience;
    let (vb, va) = (world.config.service.tune.eject_bouncer, world.config.service.tune.ejected);
    world.remember(bouncer, MemoryKind::Served, Some(attacker), salience, vb, false);
    world.remember(attacker, MemoryKind::Served, Some(bouncer), salience, va, false);
    let names_it = row_of_agent(world, bouncer).is_some_and(|r| r.event_on.contains(&ServeOutcome::Ejected));
    if names_it {
        let text = format!("{} threw {} out of {}", world.name_of(bouncer), world.name_of(attacker), world.name_of(b));
        world.push_event(EventKind::Served, &[bouncer, attacker, b], text);
    }
    true
}

/// The world-side shift-end rule (§ 2.1, cook; both tiers: `exec::actions::
/// end_shift` and `lod::stat_work`): a staffed shift of a serving row is
/// counted, and a Cook takes one staff meal from the NoodleBar's stock.
/// Nothing off.
pub fn shift_end(world: &mut World, id: EntityId) {
    if !on(world) {
        return;
    }
    let Some(j) = world.comp::<Job>(id).cloned() else { return };
    if row_for(world, j.role).is_some() {
        ownership::jobs2_row(world).serve_shifts += 1;
    }
    if j.shift_tips > 0 {
        madam_settles(world, id, &j);
    }
    // R6a: the madam's night of bookings, one line.
    crate::systems::vice::bookings_told(world, id);
    // Round 3 S5: the bouncer's night at the door, one line.
    crate::systems::vice::door_told(world, id);
    // Round 4 T4: a dancer's or a club hand's night, one line.
    crate::systems::vice::night_told(world, id);
    // Lives at work phase 4 (§ 2.3): the courier's undelivered parcels back
    // to the Yard (the loader's lifts are `lift_hour`'s).
    if street_on(world) {
        crate::systems::streetwork::parcels_back(world, id);
    }
    if !world.config.service.staff_meal || j.role != Role::Cook {
        return;
    }
    let Some(bar) =
        j.employer.filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::NoodleBar))
    else {
        return;
    };
    if world.take_stock(bar, crate::components::Good::Food, 1) == 0 {
        return;
    }
    let cfg = world.config.needs.clone();
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    crate::systems::health::ate(world, id, crate::components::MealKind::Real);
    world.mark_day(id, crate::components::trace_flags::ATE);
    ownership::jobs2_row(world).staff_meals += 1;
}

/// 8d (OPEN-123, § 2.1): the madam's cut settles at the worker's shift end,
/// `[service] madam_cut` of the shift's tips at the house, rounded once, to
/// the house's madam on duty (none on duty, or the worker herself: no cut).
/// The tally clears either way. Both tiers: a Statistical worker's tips are
/// paid by the same `encounter_row` and her shift ends in `lod::stat_work`.
fn madam_settles(world: &mut World, id: EntityId, j: &Job) {
    if let Some(job) = world.comp_mut::<Job>(id) {
        job.shift_tips = 0;
    }
    let Some(place) = j.employer else { return };
    // The house's madam on duty, else any madam it employs (the cut is the
    // house's take; her shift can end before her worker's).
    let madam = servers(world, place, ServeKind::Staff).first().map(|&(m, _)| m).or_else(|| {
        world
            .staff_of(place)
            .iter()
            .copied()
            .find(|&s| row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Staff))
    });
    let Some(m) = madam.filter(|&m| m != id) else { return };
    let cut = (j.shift_tips as f32 * world.config.service.madam_cut).round() as i64;
    if cut > 0 {
        ownership::pay(world, Some(id), Some(m), cut, Flow::MadamCut);
    }
}

/// Lives at work phase 4 (§ 2.3, loader; `lift_hour`): the shift's `n`
/// lifts at `yard` are the loader's (`Crate`, no served person: `arrive` cannot place
/// one): `served[Crate]` and `lifts` by `n`, one `Served` memory with no
/// subject (the diary's "loaded crates for <owner>"), the skill. Street only.
pub fn lift(world: &mut World, loader: EntityId, yard: EntityId, n: u32) {
    if !street_on(world) || n == 0 || !world.has::<Building>(yard) {
        return;
    }
    let Some(row) = row_of_agent(world, loader).cloned() else { return };
    {
        let r = ownership::jobs2_row(world);
        r.served[ServeKind::Crate as usize] += n;
        r.lifts += n;
    }
    let salience = world.config.service.served_salience;
    world.remember(loader, MemoryKind::Served, None, salience, row.valence, false);
    train(world, loader, row.skill, row.skill_gain * n as f32);
}

/// The regulars kept for `server` (the inspector, the diary).
pub fn regulars_of(world: &World, server: EntityId) -> &[Regular] {
    world.regulars.get(&server).map_or(&[], SmallVec::as_slice)
}
