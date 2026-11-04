use super::*;

/// The hourly pass (from `living::run`): the room ledger pruned of past
/// hours, then the madam's `Staff` encounters of the hour.
pub fn hourly(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let _t = crate::prof::scope!("service::hourly");
    let hour = world.tick / TICKS_PER_HOUR;
    world.service_room.retain(|_, r| r.hour == hour);
    staff_hour(world);
    if world.tick_of_day() == 0 {
        world.notices_today.clear();
        // The pressure pass R4: a round cut short (the shift's end, a
        // demotion) tells its notices at midnight.
        let open: Vec<EntityId> = world.notice_fold.keys().copied().collect();
        for s in open {
            notices_folded(world, s, None);
        }
    }
    if civic_on(world) {
        civic_hour(world);
    }
    if street_on(world) {
        lift_hour(world);
    }
    if tower_on(world) {
        meeting_hour(world);
        walk_in_hour(world);
        crate::systems::news::stat_beats(world);
    }
}

/// Lives at work phase 4 (§ 2.3, loader): at the last hour of a loader's
/// shift, each loader present at his Yard (a body inside, a Statistical one
/// credited by the clock: § 1.2) lifts the shift's crates (`lift`). One
/// world-side pass for both tiers.
pub(super) fn lift_hour(world: &mut World) {
    let now = world.tick;
    let mut due = Vec::new();
    for &y in world.buildings_of_kind(BuildingKind::FreightYard) {
        for &s in world.staff_of(y) {
            let last = world
                .comp::<Job>(s)
                .and_then(|j| j.shift_end(now))
                .is_some_and(|e| e > now && e - now <= TICKS_PER_HOUR);
            if last && row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Crate) && present(world, s, y) {
                due.push((y, s));
            }
        }
    }
    for (y, s) in due {
        let n = crate::systems::industry::shift_lifts(world, y);
        lift(world, s, y, n);
    }
}

/// Does `s`'s job start a shift this hour (the roster's and the headcount's
/// hour, as the madam's `staff_hour`; `Job::first_hour`)?
fn first_hour(world: &World, s: EntityId) -> bool {
    let tod = world.tick_of_day();
    world.comp::<Job>(s).is_some_and(|j| j.first_hour(tod))
}

/// Lives at work phase 3 (§ 2.2, teacher and camp warden): at the first
/// hour of a server's shift, the School's roster (every child whose place
/// today is this School, `ages::place_of`, one pass over the children) and
/// the Camp's headcount (its children), one `Pupil`/`Inmate` encounter a
/// child. Children have no `Brain`: no memory on their side, the ledger
/// names them. Civic only.
pub(super) fn civic_hour(world: &mut World) {
    let schools: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::School)
        .iter()
        .copied()
        .filter(|&s| servers(world, s, ServeKind::Pupil).iter().any(|&(t, _)| first_hour(world, t)))
        .collect();
    if !schools.is_empty() {
        let mut roster: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
        // scan-ok: once a day per School shift: the children
        for k in world.with::<Child>() {
            if !world.is_alive(k) || world.has::<Corpse>(k) {
                continue;
            }
            if let Some(crate::systems::ages::Place::School(s)) = crate::systems::ages::place_of(world, k) {
                if schools.contains(&s) {
                    roster.entry(s).or_default().push(k);
                }
            }
        }
        let hour = world.tick / TICKS_PER_HOUR;
        for (s, kids) in roster {
            let n = kids.len();
            for k in kids {
                arrive(world, k, s, ServeKind::Pupil, 0);
            }
            // S6 (OPEN-224): the class, one event a teacher (the pupils
            // taught this hour; the rest of the roster waited, untaught).
            let classes: Vec<(EntityId, u32)> = servers(world, s, ServeKind::Pupil)
                .into_iter()
                .filter_map(|(t, _)| {
                    world
                        .service_room
                        .get(&t)
                        .filter(|r| r.hour == hour && r.taken > 0)
                        .map(|r| (t, u32::from(r.taken)))
                })
                .collect();
            let taught: u32 = classes.iter().map(|&(_, m)| m).sum();
            let left = (n as u32).saturating_sub(taught);
            for (t, m) in classes {
                let (who, at) = (world.name_of(t), world.name_of(s));
                let rest = if left > 0 { format!(" ({left} more on the roll, no room)") } else { String::new() };
                let pupils = if m == 1 { "1 pupil".to_string() } else { format!("{m} pupils") };
                world.push_event(EventKind::Served, &[t, s], format!("{who} taught {pupils} at {at}{rest}"));
            }
        }
    }
    for camp in crate::systems::camp::standing_camps(world) {
        if !servers(world, camp, ServeKind::Inmate).iter().any(|&(t, _)| first_hour(world, t)) {
            continue;
        }
        let kids: Vec<EntityId> = crate::systems::camp::children_of(world, camp).to_vec();
        if kids.is_empty() {
            continue;
        }
        ownership::jobs2_row(world).camp_headcounts += 1;
        for k in kids {
            arrive(world, k, camp, ServeKind::Inmate, 0);
        }
        // Round 4 T1 (OPEN-231): the count is one line (a child's Inmate
        // encounter is no memory of the warden's, as a pupil's).
        let counting: Vec<EntityId> = servers(world, camp, ServeKind::Inmate)
            .into_iter()
            .filter(|&(t, _)| first_hour(world, t))
            .map(|(t, _)| t)
            .collect();
        crate::systems::camp::headcount(world, camp, &counting);
    }
    crate::systems::camp::abscond_hour(world);
}

/// § 2.2 (teacher): a taught School day: `teacher_floor + (1 −
/// teacher_floor) × the teacher's knowledge` onto `Child.edu_taught`
/// (`ages::on_mature` reads it under civic), and the parents who are bodies
/// at home meet the teacher at the door (a first meeting at a hashed
/// `0.1 + 0.3u`, `interacted` after; a `Served` memory each side; no walk).
pub(super) fn taught(world: &mut World, teacher: EntityId, child: EntityId, valence: f32) {
    let floor = world.config.service.teacher_floor;
    let know = world.comp::<crate::components::Skills>(teacher).map_or(0.0, |s| s.knowledge);
    if let Some(c) = world.comp_mut::<Child>(child) {
        c.edu_taught += floor + (1.0 - floor) * know;
    }
    ownership::jobs2_row(world).taught_days += 1;
    let Some(home) = world.comp::<Household>(child).and_then(|h| h.home) else { return };
    let salience = world.config.service.served_salience;
    for p in crate::systems::camp::parents_of(world, child) {
        let at_home = world.comp::<Position>(p).is_some_and(|x| x.building == Some(home))
            && world.comp::<Brain>(p).is_some_and(|b| b.lod != Lod::Statistical);
        if !at_home || p == teacher {
            continue;
        }
        if world.edge(p, teacher).is_some() {
            crate::systems::social::interacted(world, p, teacher);
        } else {
            let u = hash(world, PURPOSE_REGULAR, key(teacher), key(p));
            crate::systems::social::first_meeting(world, p, teacher, 0.1 + 0.3 * u);
        }
        world.remember(p, MemoryKind::Served, Some(teacher), salience, valence, false);
        world.remember(teacher, MemoryKind::Served, Some(p), salience, valence, false);
    }
}

/// § 2.2 (super): the stop's encounter with a tenant: in arrears, a
/// `Notice` (once a tenant a day; a `Served` memory at −0.3; the event
/// when the row names it); a whole coin of rent due and the purse holding
/// it, the rent settled at the door (`ownership::settle_rent`, no arrears
/// clock: the midnight settle moved earlier, the same `Flow::Rent`).
pub(super) fn tenant(world: &mut World, server: EntityId, tenant: EntityId, block: EntityId) -> Outcome {
    let Some((arrears, due)) = world.comp::<Household>(tenant).map(|h| (h.arrears, h.rent_due.floor() as i64)) else {
        return Outcome::Done;
    };
    let day = world.day();
    let mut outcome = Outcome::Done;
    if arrears > 0 && world.notices_today.insert((tenant, day)) {
        outcome = Outcome::Notice;
        let salience = world.config.service.served_salience;
        let v = world.config.service.tune.notice;
        world.remember(tenant, MemoryKind::Served, Some(server), salience, v, false);
        ownership::jobs2_row(world).notices += 1;
    }
    if due >= 1 && world.purse(Some(tenant)) >= due {
        let before = world.comp::<Household>(tenant).map_or(0.0, |h| h.rent_due);
        ownership::settle_rent(world, tenant, block, false);
        let after = world.comp::<Household>(tenant).map_or(0.0, |h| h.rent_due);
        let paid = (before - after).round() as i64;
        if paid > 0 {
            ownership::jobs2_row(world).door_rent += paid;
        }
    }
    outcome
}

/// § 2.2 (super): the round's arrival at a Block (`trades::round_plan` for
/// a body, `lod::stat_work` for a Statistical super at the shift's first
/// hour): every adult resident but the super is served `Tenant` by the
/// super himself (his row's queue is the round, not his post). Civic only.
pub fn super_stop(world: &mut World, id: EntityId, block: EntityId) {
    if !civic_on(world) || !row_of_agent(world, id).is_some_and(|r| r.kind == ServeKind::Tenant) {
        return;
    }
    ownership::jobs2_row(world).super_stops += 1;
    let tenants: Vec<EntityId> = world
        .residents_of(block)
        .iter()
        .copied()
        .filter(|&t| {
            t != id
                && world.has::<Brain>(t)
                && crate::systems::demography::is_adult(world, t)
                && crate::systems::law::living(world, t)
        })
        .collect();
    // The pre-view pass phase 4 (P3, OPEN-120): only a tenant present and
    // awake is served (a body in the Block, a Statistical one on the home
    // draw); the rest wait for the next round (`presence_missed`).
    let presence = crate::systems::presence::on(world);
    let key = u64::from(id.index);
    // The pressure pass R4 (OPEN-219): the stop's notices go into the
    // round's one line (`notices_folded`), not a line a tenant.
    world.notice_fold.entry(id).or_default();
    for t in tenants {
        if presence && !crate::systems::presence::met_at_home(world, t, block, key) {
            crate::systems::presence::missed(world);
            continue;
        }
        serve(world, id, t, block, ServeKind::Tenant, 0);
    }
}

/// Round 3 S4 (OPEN-216): a super's leg at his post, the stations block
/// that `started` just worked, named for one of `[presence] super_legs`
/// (a draw keyed on the super and the block): a repair in a tenant's flat
/// (any adult tenant), a tenant at home stopping him in the hall with a
/// complaint (present: a body in the Block, a Statistical one on the home
/// draw), or the stairs walked. One event, the tenant an actor; nothing
/// for anyone but a super inside his post.
pub fn super_leg(world: &mut World, id: EntityId, started: Tick) {
    use crate::config::SuperLeg;
    let Some(c) = world.config.presence.clone() else { return };
    if c.super_legs.is_empty()
        || !civic_on(world)
        || !row_of_agent(world, id).is_some_and(|r| r.kind == ServeKind::Tenant)
    {
        return;
    }
    let Some(post) = world.comp::<Job>(id).and_then(|j| j.employer) else { return };
    if world.comp::<crate::components::Position>(id).and_then(|p| p.building) != Some(post) {
        return;
    }
    let k = key(id) ^ started.rotate_left(20);
    let pick = |u: f32, n: usize| ((u * n as f32) as usize).min(n.saturating_sub(1));
    let leg = c.super_legs[pick(hash(world, PURPOSE_LEG, k, 0), c.super_legs.len())];
    // A tenant he saw to inside `super_leg_days` (a Served memory between
    // them) waits his turn: one complaint a week, not one a morning.
    let since = world.tick.saturating_sub(u64::from(c.super_leg_days) * crate::time::TICKS_PER_DAY);
    let seen = |w: &World, t: EntityId| {
        w.comp::<crate::components::Memory>(id).is_some_and(|m| {
            m.entries.iter().any(|e| e.kind == MemoryKind::Served && e.subject == Some(t) && e.tick >= since)
        })
    };
    let tenants: Vec<EntityId> = world
        .residents_of(post)
        .iter()
        .copied()
        .filter(|&t| {
            t != id
                && world.has::<Brain>(t)
                && crate::systems::demography::is_adult(world, t)
                && crate::systems::law::living(world, t)
                && !seen(world, t)
        })
        .collect();
    let (who, at) = (world.name_of(id), world.name_of(post));
    let floors = world.comp::<Building>(post).map_or(1, |bd| bd.floors.max(1));
    let stairs =
        format!("{who} walked the stairs and halls of {at} ({floors} floor{})", if floors == 1 { "" } else { "s" });
    let (text, tenant) = match leg {
        SuperLeg::Repair if !tenants.is_empty() && !c.super_repairs.is_empty() => {
            let t = tenants[pick(hash(world, PURPOSE_LEG, k, 1), tenants.len())];
            let what = &c.super_repairs[pick(hash(world, PURPOSE_LEG, k, 2), c.super_repairs.len())];
            (format!("{who} fixed {what} in {}'s flat at {at}", world.name_of(t)), Some(t))
        }
        SuperLeg::Complaint if !c.super_complaints.is_empty() => {
            let present: Vec<EntityId> =
                tenants.iter().copied().filter(|&t| crate::systems::presence::met_at_home(world, t, post, k)).collect();
            if present.is_empty() {
                (stairs, None)
            } else {
                let t = present[pick(hash(world, PURPOSE_LEG, k, 1), present.len())];
                let what = &c.super_complaints[pick(hash(world, PURPOSE_LEG, k, 2), c.super_complaints.len())];
                (format!("{} stopped {who} in the hall of {at} about {what}", world.name_of(t)), Some(t))
            }
        }
        _ => (stairs, None),
    };
    let mut actors = vec![id];
    actors.extend(tenant);
    actors.push(post);
    world.push_event(EventKind::Served, &actors, text);
    if let Some(t) = tenant {
        let salience = world.config.service.served_salience;
        world.remember(id, MemoryKind::Served, Some(t), salience, 0.0, false);
        world.remember(t, MemoryKind::Served, Some(id), salience, 0.0, false);
    }
}

const PURPOSE_LEG: u64 = 0x5e52_0020;

/// The pressure pass R4 (OPEN-219): the round's one `Notice` line, at its
/// end (`trades::round_plan`, `lod`'s Statistical round; `stops` the
/// round's Blocks) or at midnight for a round cut short (`None`): the count
/// and the first `[noise] notice_names` tenants; every tenant noticed is an
/// actor (his diary keeps the line). Nothing when the round noticed nobody.
pub fn notices_folded(world: &mut World, id: EntityId, stops: Option<usize>) {
    let Some(tenants) = world.notice_fold.remove(&id) else { return };
    if tenants.is_empty() {
        return;
    }
    let names = usize::from(world.config.noise.notice_names);
    let shown: Vec<String> = tenants.iter().take(names).map(|&t| world.name_of(t)).collect();
    let more = tenants.len().saturating_sub(shown.len());
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    let round = match stops {
        Some(n) => format!("the round of {n} Blocks"),
        None => "a round cut short".to_string(),
    };
    let n = tenants.len();
    let who = world.name_of(id);
    let s = if n == 1 { "" } else { "s" };
    let text = format!("{who} served notice on {n} tenant{s} in arrears on {round} ({}{tail})", shown.join(", "));
    let mut actors = vec![id];
    actors.extend(tenants);
    world.push_event(EventKind::Notice, &actors, text);
}

/// § 2.2 (jailer): a breakout past the jailers on duty: a `Served` memory
/// at −0.3 each (subject none), the first named in the `Jailbreak` text
/// (`raid::breach`). Civic only.
pub fn jailers_passed(world: &mut World, jail: EntityId) -> Option<EntityId> {
    if !civic_on(world) {
        return None;
    }
    let on_duty = servers_on_duty(world, jail, ServeKind::Inmate);
    let salience = world.config.service.served_salience;
    let v = world.config.service.tune.jailbreak;
    for &j in &on_duty {
        world.remember(j, MemoryKind::Served, None, salience, v, false);
    }
    on_duty.first().copied()
}

/// § 2.1 (madam): at the first hour of a worker's shift at a place with a
/// `Staff`-kind server on duty, the worker is served (the madam's house).
fn staff_hour(world: &mut World) {
    let tod = world.tick_of_day();
    let places: Vec<EntityId> = world
        .config
        .serves
        .iter()
        .filter(|r| r.kind == ServeKind::Staff)
        .flat_map(|_| world.buildings_of_kind(BuildingKind::Brothel).iter().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for place in places {
        if servers(world, place, ServeKind::Staff).is_empty() {
            continue;
        }
        let workers: Vec<EntityId> = world
            .staff_of(place)
            .iter()
            .copied()
            .filter(|&s| {
                row_of_agent(world, s).is_some_and(|r| r.kind != ServeKind::Staff)
                    && world.comp::<Job>(s).is_some_and(|j| j.first_hour(tod))
                    && present(world, s, place)
            })
            .collect();
        for w in workers {
            arrive(world, w, place, ServeKind::Staff, 0);
        }
    }
}
