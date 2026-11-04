use super::*;

/// A Vice venue kind.
pub fn is_vice_kind(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::StripClub | BuildingKind::Brothel)
}

/// A visit's price at a Vice venue by its tier (`venue_price`'s base).
pub fn visit_price(world: &World, kind: BuildingKind, tier: u8) -> i64 {
    let t = usize::from(tier.min(2));
    match kind {
        BuildingKind::StripClub => world.config.vice.strip_price[t],
        BuildingKind::Brothel => world.config.vice.brothel_price[t],
        _ => 0,
    }
}

/// The fun a visit gives (`leisure::gain_at`).
pub fn gain(world: &World, kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::StripClub => world.config.vice.gain_strip,
        BuildingKind::Brothel => world.config.vice.gain_brothel,
        _ => 0.0,
    }
}

/// The ladder's weight on a Brothel: `1 + intimacy_pull × (1 − intimacy)`
/// (1 elsewhere).
pub fn score_mult(world: &World, id: EntityId, kind: BuildingKind) -> f32 {
    if kind != BuildingKind::Brothel {
        return 1.0;
    }
    let i = world.comp::<Needs>(id).map_or(0.5, |n| n.intimacy);
    1.0 + world.config.vice.intimacy_pull * (1.0 - i).clamp(0.0, 1.0)
}

/// The tipped worker on duty at `b` (the lowest id): a `tipped` row's
/// staff on shift now, inside it (a Statistical one counts as there), not held.
pub fn tipped_on_duty(world: &World, b: EntityId) -> Option<EntityId> {
    tipped_for(world, b, EntityId::NONE)
}

/// Shadow V4 bug 7: the tipped worker on duty a visit's tip goes to, turn
/// about by the visitor and the hour (every tip went to the lowest id on
/// duty: 56 c against 0 c on the same shifts).
fn tipped_for(world: &World, b: EntityId, visitor: EntityId) -> Option<EntityId> {
    let tod = world.tick_of_day();
    let on_duty: Vec<EntityId> = world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod) && crate::systems::trades::row_of(world, j.role).is_some_and(|r| r.tipped)
            }) && !world.has::<Sentence>(s)
                && world.comp::<Brain>(s).is_some_and(|br| {
                    br.lod == Lod::Statistical || world.comp::<Position>(s).is_some_and(|p| p.building == Some(b))
                })
        })
        .collect();
    if on_duty.is_empty() {
        return None;
    }
    let turn = u64::from(visitor.index) + world.tick / crate::time::TICKS_PER_HOUR;
    Some(on_duty[(turn % on_duty.len() as u64) as usize])
}

pub(super) fn add_intimacy(world: &mut World, id: EntityId, gain: f32) {
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.intimacy = (n.intimacy + gain).min(1.0);
    }
}

/// A paid venue visit done (`leisure::on_complete`'s `Enjoy`, the
/// Statistical evening): at a StripClub or a Brothel, the counts, the
/// Brothel's intimacy and the tip; at the Lounge with a companion on duty,
/// a companion's visit (intimacy, the tip).
pub fn on_visit(world: &mut World, visitor: EntityId, b: EntityId, price: i64) {
    if !on(world) {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let worker = tipped_for(world, b, visitor);
    match kind {
        BuildingKind::StripClub => {
            let row = ownership::jobs2_row(world);
            row.strip_visits += 1;
            row.vice_revenue += price;
        }
        // Jobs & lives phase 5: counted by `dollhouse::on_visit`; no tip.
        BuildingKind::Dollhouse => return,
        BuildingKind::Brothel => {
            let row = ownership::jobs2_row(world);
            row.brothel_visits += 1;
            row.vice_revenue += price;
            add_intimacy(world, visitor, world.config.vice.intimacy_gain);
        }
        BuildingKind::Lounge if worker.is_some() => {
            ownership::jobs2_row(world).companion_visits += 1;
            add_intimacy(world, visitor, world.config.vice.intimacy_gain);
        }
        // Shadow V4 bug 7: a Club's dancer is tipped too (it fell through).
        BuildingKind::Club if worker.is_some() => {}
        _ => return,
    }
    // Lives at work phase 1 (§ 2.1): the tip goes to the server the
    // encounter names (`service::serve`), not the turn-about worker.
    if crate::systems::service::on(world) {
        return;
    }
    let Some(w) = worker.filter(|&w| w != visitor) else { return };
    let tip = ((price as f32 * world.config.vice.tip_share).round() as i64).max(1);
    ownership::pay(world, Some(visitor), Some(w), tip, Flow::Tip);
}

// ---------------------------------------------------------------------------
// The madam's bookings (the pressure pass R6a, OPEN-217)
// ---------------------------------------------------------------------------

/// `service::serve`: a Brothel visit served by `worker` while a madam is on
/// duty at the house is her booking: tallied for her shift's line
/// (`bookings_told`), and on `[trade_verbs] rough_p x (1 - lawfulness)` (a
/// hash of the pair and the night) the client turns rough: the worker hurt
/// (safety, a `Lost` memory, an Assault raised on him), the madam throws
/// him out and bars him from the house for `rough_ban_days` (`barred`),
/// her grudge on him, the event. The madam had no client in her diary (V10:
/// "a madam who manages nobody"); her cut of the tips was the only trace.
/// Both tiers (the serve is world-side). Nothing with `[trade_verbs]` absent.
pub fn booked(world: &mut World, worker: EntityId, client: EntityId, house: EntityId, coins: i64) {
    let Some(c) = world.config.trade_verbs.clone() else { return };
    if !on(world) || world.comp::<Building>(house).is_none_or(|b| b.kind != BuildingKind::Brothel) {
        return;
    }
    let Some(madam) = crate::systems::service::servers_on_duty(world, house, crate::components::ServeKind::Staff)
        .into_iter()
        .find(|&m| m != worker && m != client)
    else {
        return;
    };
    let night = world.day();
    let lawful = world.comp::<crate::components::Personality>(client).map_or(0.5, |p| p.lawfulness);
    let rough = hash(world, PURPOSE_ROUGH, key(client) ^ key(worker), night) < c.rough_p * (1.0 - lawful).max(0.0);
    {
        let b = world.jobs2.vice.bookings.entry(key(madam)).or_default();
        if b.house != house {
            *b = Bookings { house, ..Bookings::default() };
        }
        b.clients += 1;
        b.coins += coins.max(0);
        match b.workers.iter_mut().find(|(w, _)| *w == worker) {
            Some((_, n)) => *n += 1,
            None => b.workers.push((worker, 1)),
        }
        b.rough += u32::from(rough);
    }
    if !rough {
        return;
    }
    if let Some(n) = world.comp_mut::<Needs>(worker) {
        n.safety = (n.safety - 0.3).max(0.0);
    }
    world.remember(worker, MemoryKind::Lost, Some(client), 0.6, -0.5, false);
    crate::systems::grudges::add(world, madam, client, crate::word::GrudgeCause::Assaulted, 0.3, 0);
    let tile = world.comp::<Building>(house).map(|b| b.door).unwrap_or_default();
    crate::systems::law::raise_crime_on(
        world,
        client,
        Some(worker),
        Some(madam),
        crate::components::Crime::Assault,
        tile,
    );
    let until = night + c.rough_ban_days;
    world.jobs2.vice.barred.insert(key(client), (house, until));
    if world.comp::<Position>(client).is_some_and(|p| p.building == Some(house)) {
        world.abort_plan(client);
        world.leave_building(client);
    }
    let text = format!(
        "{} roughed up {} at {}; {} threw him out and barred him from the house",
        world.name_of(client),
        world.name_of(worker),
        world.name_of(house),
        world.name_of(madam)
    );
    world.push_event(EventKind::Served, &[madam, client, worker, house], text);
}

/// Round 3 S5: bar `id` from the house `b` until the day `until` (the
/// bouncer's rough client, `customers::rough_at_door`).
pub fn bar(world: &mut World, id: EntityId, b: EntityId, until: u64) {
    world.jobs2.vice.barred.insert(key(id), (b, until));
}

/// Round 4 T7 (OPEN-233; `service::refused`): `id` is turned away at `b`
/// now; was he turned away there inside `window` ticks before (a return)?
/// The ledger keeps each face's refusals that long (one door a face was all
/// `Brain.refused` held: a drunk refused at two doors in turn came back to
/// each).
pub fn turned_again(world: &mut World, id: EntityId, b: EntityId, window: u64) -> bool {
    let (now, day) = (world.tick, world.day());
    let ledger = &mut world.jobs2.vice.turned;
    ledger.retain(|_, v| {
        v.retain(|&(_, at, until)| now.saturating_sub(at) < window || day < until);
        !v.is_empty()
    });
    let list = ledger.entry(key(id)).or_default();
    let prev = list.iter().position(|&(d, _, _)| d == b);
    let until = prev.map_or(0, |i| list.remove(i).2);
    list.push((b, now, until));
    prev.is_some()
}

/// Round 4 T7: the return `id` made to `b` bars him there until the day
/// `until` (the ledger holds a bar a door; `bar`'s one a face is R6a's).
pub fn shut_out(world: &mut World, id: EntityId, b: EntityId, until: u64) {
    if let Some(e) = world.jobs2.vice.turned.get_mut(&key(id)).and_then(|v| v.iter_mut().find(|e| e.0 == b)) {
        e.2 = e.2.max(until);
    }
}

/// R6a: is `id` barred from the house `b` today (a rough booking's; round
/// 4 T7: or a return's, `shut_out`)?
pub fn barred(world: &World, id: EntityId, b: EntityId) -> bool {
    let day = world.day();
    world.jobs2.vice.barred.get(&key(id)).is_some_and(|&(h, until)| h == b && day < until)
        || world.jobs2.vice.turned.get(&key(id)).is_some_and(|v| v.iter().any(|&(d, _, u)| d == b && day < u))
}

/// R6a: the madam's shift ends (`service::shift_end`): her night's bookings
/// told in one line (the clients, the house's take, her busiest workers by
/// name, the rough ones thrown out), the tally cleared; the lifted bars dropped.
pub fn bookings_told(world: &mut World, madam: EntityId) {
    let Some(names) = world.config.trade_verbs.as_ref().map(|c| c.madam_names) else { return };
    let today = world.day();
    world.jobs2.vice.barred.retain(|_, &mut (_, until)| today < until);
    let Some(mut b) = world.jobs2.vice.bookings.remove(&key(madam)) else { return };
    if b.clients == 0 {
        return;
    }
    b.workers.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let named: Vec<String> = b.workers.iter().take(names).map(|&(w, n)| format!("{} {n}", world.name_of(w))).collect();
    let more = b.workers.len().saturating_sub(names);
    let rough = if b.rough > 0 { format!("; {} rough, thrown out", b.rough) } else { String::new() };
    let text = format!(
        "{} booked {} client{} at {} tonight ({}{}), {}c through the house{rough}",
        world.name_of(madam),
        b.clients,
        if b.clients == 1 { "" } else { "s" },
        world.name_of(b.house),
        named.join(", "),
        if more > 0 { format!(" and {more} more") } else { String::new() },
        b.coins
    );
    let mut subjects = vec![madam, b.house];
    subjects.extend(b.workers.iter().take(names).map(|&(w, _)| w));
    world.push_event(EventKind::Served, &subjects, text);
}
