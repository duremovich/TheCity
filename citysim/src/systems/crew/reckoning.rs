use super::*;

pub(super) fn missed(world: &mut World, c: &CrewCfg, m: EntityId, d: &Duty) {
    let day = world.day() as u32;
    let window = c.miss_window_days;
    // R5 (OPEN-214), the attrition judged: a court summons missed at work is
    // excused (the standing, not the colours).
    if c.court_work_excused && d.kind == DutyKind::Court && d.chose_job {
        book(world, m).standing -= c.miss_standing;
        let text = format!(
            "{} missed {}'s court summons at {} (at work: excused)",
            world.name_of(m),
            gang_name(world, d.gang),
            world.name_of(d.place)
        );
        world.push_event(EventKind::Crew, &[m, d.gang], text);
        ownership::jobs2_row(world).crew_duty_missed += 1;
        return;
    }
    let count = {
        let b = book(world, m);
        b.misses.retain(|&x| x + window > day);
        b.misses.push(day);
        b.standing -= c.miss_standing;
        b.misses.len() as u32
    };
    let why = if d.chose_job { " (at work)" } else { "" };
    let text = format!(
        "{} missed {}'s {} at {}{why} ({count} in {window} days)",
        world.name_of(m),
        gang_name(world, d.gang),
        d.kind.label(),
        world.name_of(d.place)
    );
    world.push_event(EventKind::Crew, &[m, d.gang], text);
    ownership::jobs2_row(world).crew_duty_missed += 1;
    if count >= c.colours_misses {
        if let Some(b) = world.crew.books.get_mut(&m) {
            b.misses.clear();
        }
        send_reckoning(world, c, m, d.gang, CallWhy::Colours(count));
    }
}

/// The gang's toughest free member, not `not`, with no errand.
pub(super) fn enforcer(world: &World, gang: EntityId, not: EntityId) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    g.members
        .iter()
        .copied()
        .filter(|&m| m != not && crate::systems::contracts::free_adult(world, m))
        .filter(|&m| !world.visits.contains_key(&m) && !world.crew.calls.contains_key(&m))
        .map(|m| (std::cmp::Reverse(ordered_float::OrderedFloat(crate::systems::combat::power(world, m))), m))
        .min()
        .map(|(_, m)| m)
}

/// Send the toughest to `target` (a person Visit at the next hour), or with
/// nobody to send, take the colours where he stands.
pub(super) fn send_reckoning(world: &mut World, c: &CrewCfg, target: EntityId, gang: EntityId, why: CallWhy) {
    let Some(e) = enforcer(world, gang, target) else {
        out_of_colours(world, target, gang, why, None);
        return;
    };
    let call = Call { target, gang, why };
    let hour = (world.tick_of_day() / TICKS_PER_HOUR as u16 + 1) as u8;
    if !crate::systems::household::on(world) || hour >= 23 {
        reckoning(world, c, e, call, false);
        return;
    }
    world.visits.insert(e, PendingVisit { kin: target, why: VisitWhy::Reckoning, hour });
    world.crew.calls.insert(e, call);
}

/// The reckoning's Visit arrived (`household::credit`), or it is met off
/// screen at its hour.
pub fn reckoned(world: &mut World, enforcer: EntityId, target: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(call) = world.crew.calls.remove(&enforcer) else { return };
    if call.target != target {
        return;
    }
    reckoning(world, &c, enforcer, call, walked);
}

pub(super) fn reckoning(world: &mut World, c: &CrewCfg, enforcer: EntityId, call: Call, walked: bool) {
    let target = call.target;
    if world.gang_of(target) != Some(call.gang) || !law::living(world, target) || world.has::<Sentence>(target) {
        return;
    }
    if !law::living(world, enforcer) || world.has::<Sentence>(enforcer) {
        out_of_colours(world, target, call.gang, call.why, None);
        return;
    }
    let mods = law::FightMods { kill_mult: c.beat_kill_mult, ..Default::default() };
    // C3a: live when both are Full; the tail is `reckoning_end`.
    let tail = crate::systems::combat::Tail::Reckoning(Box::new(Reckon { call, walked }));
    crate::systems::combat::fight(world, enforcer, target, mods, tail);
}

/// C3a: a reckoning being fought, for its tail.
#[derive(Clone, Debug)]
pub struct Reckon {
    pub call: Call,
    pub walked: bool,
}

/// C3a, the reckoning's tail: the line, the crime, the colours lost.
pub(crate) fn reckoning_end(world: &mut World, x: &Reckon, c: &crate::systems::combat::Clash) {
    let (enforcer, target, walked, call) = (c.a, c.b, x.walked, x.call.clone());
    let (winner, loser, died) = (c.winner, c.loser, c.died);
    let (en, tn) = (world.name_of(enforcer), world.name_of(target));
    let how = if walked { "came for" } else { "caught up with" };
    let fight = if winner == enforcer {
        if died {
            format!("{en} {how} {tn} and beat them to death")
        } else {
            format!("{en} {how} {tn} and beat them")
        }
    } else {
        format!("{en} {how} {tn}, who fought back and won")
    };
    let kind = if died && loser == target { EventKind::Murder } else { EventKind::Assault };
    world.push_event(kind, &[enforcer, target], fight);
    let tile = world.comp::<Position>(target).or_else(|| world.comp::<Position>(enforcer)).map(|p| p.tile);
    if let Some(t) = tile {
        let crime = if died && loser == target { Crime::Murder } else { Crime::Assault };
        if law::living(world, enforcer) {
            law::raise_crime_on(world, enforcer, (!died).then_some(target), Some(target), crime, t);
        }
    }
    if law::living(world, target) {
        let by = (winner == enforcer).then_some(enforcer);
        out_of_colours(world, target, call.gang, call.why, by);
    }
}

fn out_of_colours(world: &mut World, target: EntityId, gang: EntityId, why: CallWhy, by: Option<EntityId>) {
    let reason = match why {
        CallWhy::Talked => "talked to the law: thrown out".to_string(),
        CallWhy::Colours(n) => format!("lost the colours: {n} duties missed"),
    };
    let gname = gang_name(world, gang);
    let by_text = by.map_or_else(String::new, |b| format!(", beaten out by {}", world.name_of(b)));
    let text = format!("{} is out of {gname} ({reason}{by_text})", world.name_of(target));
    let mut actors = vec![target, gang];
    actors.extend(by);
    world.push_event(EventKind::Crew, &actors, text);
    {
        let row = ownership::jobs2_row(world);
        match why {
            CallWhy::Talked => row.crew_talker_beaten += 1,
            CallWhy::Colours(_) => row.crew_colours_lost += 1,
        }
    }
    world.crew.books.remove(&target);
    crate::systems::gang::leave(world, target, &reason);
    // R5 (OPEN-214): beaten out is a leaver's cooldown, as walking away is
    // (a shadow pick beaten out at 11:54 joined the same gang at 17:45).
    let until = world.tick + world.config.living.desist_cooldown_days * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<crate::components::Brain>(target) {
        b.cooldowns.insert(crate::components::GoalKind::JoinGang, until);
    }
}

/// A jailed member's talk, once a sentence of `talk_min_days` or more: a
/// report he files on a gang-mate (the leader first) for the gang's trade.
pub(super) fn talks(world: &mut World, c: &CrewCfg) {
    let now = world.tick;
    let mut talkers = Vec::new();
    for gang in world.gangs() {
        let Some(g) = world.comp::<Gang>(gang) else { continue };
        for &m in &g.members {
            let Some(s) = world.comp::<Sentence>(m) else { continue };
            if matches!(s.crime, Crime::Vagrancy | Crime::FareEvasion) {
                continue;
            }
            if s.until_tick.saturating_sub(now) < u64::from(c.talk_min_days) * TICKS_PER_DAY {
                continue;
            }
            if world.crew.books.get(&m).is_some_and(|b| b.talk_rolled) {
                continue;
            }
            talkers.push((m, gang));
        }
    }
    for (m, gang) in talkers {
        book(world, m).talk_rolled = true;
        let loyalty = world.comp::<Personality>(m).map_or(0.5, |p| p.loyalty);
        let p = c.talk_p * (1.0 - loyalty).max(0.0);
        if hash(world, PURPOSE_TALK, m, now / TICKS_PER_DAY) >= p {
            continue;
        }
        let (leader, members) =
            world.comp::<Gang>(gang).map(|g| (g.leader, g.members.clone())).unwrap_or((None, Vec::new()));
        let named = leader
            .filter(|&l| l != m)
            .or_else(|| members.iter().copied().find(|&x| x != m && !world.has::<Sentence>(x)));
        let Some(named) = named else { continue };
        book(world, m).talked = true;
        law::file_report(world, Crime::Extortion, named, Some(m));
        let text = format!(
            "{} talked in the cells: named {} of {} for Extortion",
            world.name_of(m),
            world.name_of(named),
            gang_name(world, gang)
        );
        world.push_event(EventKind::Crew, &[m, named, gang], text);
        ownership::jobs2_row(world).crew_talked += 1;
    }
}
