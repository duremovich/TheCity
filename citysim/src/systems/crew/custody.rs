use super::*;

/// The gang that would take `gang` in: its rival if bigger, else the
/// biggest gang with no creed whose Hideout is nearest.
pub(super) fn absorber(world: &World, gang: EntityId) -> Option<EntityId> {
    let own = world.comp::<Gang>(gang)?.members.len();
    let at = world.hideout_of(gang).and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
    let ok = |w: &World, g: EntityId| {
        g != gang && w.comp::<Gang>(g).is_some_and(|x| x.creed.is_none() && x.members.len() >= own.max(2))
    };
    if let Some(r) = world.rival_of(gang).filter(|&r| ok(world, r)) {
        return Some(r);
    }
    world
        .gangs()
        .into_iter()
        .filter(|&g| ok(world, g))
        .filter_map(|g| {
            let d = world.hideout_of(g).and_then(|h| world.comp::<Building>(h)).map(|b| b.door)?;
            Some((d.manhattan(at), g))
        })
        .min()
        .map(|(_, g)| g)
}

// ---------------------------------------------------------------------------
// Release and return
// ---------------------------------------------------------------------------

/// `gang::on_member_arrested`: the sentence's start.
pub fn on_arrested(world: &mut World, id: EntityId) {
    if !on(world) || world.gang_of(id).is_none() {
        return;
    }
    let now = world.tick;
    let b = book(world, id);
    b.jailed_at = Some(now);
    b.talked = false;
    b.talk_rolled = false;
}

/// The gang paid `coins` of bail for `id` (`protection::daily`).
pub fn note_bail(world: &mut World, id: EntityId, coins: i64) {
    if cfg(world).is_some_and(|c| c.bail_debt) && coins > 0 {
        book(world, id).owed += coins;
    }
}

/// Phase 5a's hook: `id` talked to the law inside (a jail contact).
pub fn note_talked(world: &mut World, id: EntityId) {
    if on(world) && world.gang_of(id).is_some() {
        book(world, id).talked = true;
    }
}

/// He talked: his own flag, or a report he filed names a gang-mate since
/// his sentence began.
pub(super) fn talked(world: &World, id: EntityId, gang: EntityId) -> bool {
    let b = world.crew.books.get(&id);
    if b.is_some_and(|b| b.talked) {
        return true;
    }
    let since = b.and_then(|b| b.jailed_at).unwrap_or(0);
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    world
        .crime_reports()
        .iter()
        .any(|r| r.witness == Some(id) && r.tick >= since && r.suspect != id && members.contains(&r.suspect))
}

/// `gang::on_member_released`: the gang's side of the release. A talker is
/// met by the toughest; the rest walk to the Hideout to be welcomed.
pub fn on_released(world: &mut World, id: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(gang) = world.gang_of(id) else { return };
    if talked(world, id, gang) {
        send_reckoning(world, &c, id, gang, CallWhy::Talked);
        return;
    }
    // A night in the cells is no homecoming.
    let now = world.tick;
    let served = world.crew.books.get(&id).and_then(|b| b.jailed_at).map_or(0, |t| now.saturating_sub(t));
    if served < TICKS_PER_DAY {
        if let Some(b) = world.crew.books.get_mut(&id) {
            b.jailed_at = None;
            b.talk_rolled = false;
        }
        return;
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let hour = (world.tick_of_day() / TICKS_PER_HOUR as u16) as u8 + c.welcome_after_hours;
    let day = world.day();
    let duty = Duty { gang, kind: DutyKind::Welcome, place: hideout, hour, day, chose_job: false, kept: false };
    world.crew.duties.insert(id, duty);
    if !crate::systems::household::on(world) || hour >= 22 {
        arrived(world, id, hideout, false);
        return;
    }
    world.visits.insert(id, PendingVisit { kin: hideout, why: VisitWhy::Crew, hour });
}

pub(super) fn welcome(
    world: &mut World,
    c: &CrewCfg,
    id: EntityId,
    gang: EntityId,
    place: EntityId,
    crew: &[EntityId],
    walked: bool,
) {
    let now = world.tick;
    let served =
        world.crew.books.get(&id).and_then(|b| b.jailed_at).map_or(0, |t| now.saturating_sub(t) / TICKS_PER_DAY);
    let owed = world.crew.books.get(&id).map_or(0, |b| b.owed);
    let rank_before = world.comp::<GangMember>(id).map_or(0, |x| x.rank);
    if served >= u64::from(c.talk_min_days) {
        book(world, id).standing += c.served_standing;
        crate::systems::gang::recompute_leader(world, gang);
    }
    if let Some(b) = world.crew.books.get_mut(&id) {
        b.jailed_at = None;
        b.talk_rolled = false;
    }
    let rank_after = world.comp::<GangMember>(id).map_or(0, |x| x.rank);
    for &o in crew {
        crate::systems::social::interacted(world, id, o);
    }
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.belonging = (n.belonging + 0.5).min(1.0);
    }
    world.remember(id, crate::components::MemoryKind::Socialised, Some(gang), 0.6, 0.4, false);
    let gname = gang_name(world, gang);
    let who = if crew.is_empty() {
        if walked {
            " (nobody there to meet them)".to_string()
        } else {
            String::new()
        }
    } else {
        format!(" by {}", names_of(world, crew))
    };
    let debt = if owed > 0 { format!("; owes {owed}c for the bail") } else { String::new() };
    let rank = if rank_after > rank_before {
        if rank_after >= 2 {
            "; he leads now".to_string()
        } else {
            "; made lieutenant".to_string()
        }
    } else if served >= u64::from(c.talk_min_days) {
        format!("; {served} days served, nothing said inside")
    } else {
        String::new()
    };
    let text = format!("{} welcomed back to {gname} at {}{who}{debt}{rank}", world.name_of(id), world.name_of(place));
    let mut actors = vec![id, gang, place];
    actors.extend(crew.iter().copied());
    world.push_event(EventKind::Crew, &actors, text);
    let row = ownership::jobs2_row(world);
    row.crew_welcomes += 1;
    if rank_after > rank_before {
        row.crew_promoted += 1;
    }
}
