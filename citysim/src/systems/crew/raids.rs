use super::*;

pub(super) fn post_gang(world: &mut World, c: &CrewCfg, gang: EntityId) {
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (order, leader, raid_at, members) = (g.order, g.leader, g.raid_at, g.members.clone());
    let Some(hideout) = world.hideout_of(gang) else { return };
    if world.comp::<Gang>(gang).is_some_and(|g| g.is_sacked(world.tick)) {
        return;
    }
    let mut free: Vec<EntityId> =
        members.iter().copied().filter(|&m| Some(m) != leader && free_for_duty(world, m)).collect();
    // A fair rota: a keyed shuffle a day.
    let day = world.day();
    free.sort_by(|&a, &b| {
        hash(world, PURPOSE_PICK, a, day).total_cmp(&hash(world, PURPOSE_PICK, b, day)).then(a.cmp(&b))
    });
    // Court: the body leader summons by name.
    if let (Some(_), Some(lc)) = (crate::systems::leaders::body_leader(world, gang), world.config.leaders.clone()) {
        let mut n = 0;
        let mut i = 0;
        while i < free.len() && n < usize::from(c.court_summons) {
            let m = free[i];
            if in_reach(world, c, m, hideout) {
                free.remove(i);
                post(world, c, m, gang, DutyKind::Court, hideout, u8::try_from(lc.court_hour).unwrap_or(20));
                n += 1;
            } else {
                i += 1;
            }
        }
    }
    // The city milestone C9b: the seat's standing guard, tonight's watch first
    // (then the rota), of those within reach of the seat, under every order
    // (lying low too); kept out of the muster, holding the night there.
    let n = crate::systems::vertical::seat_guard_n(world);
    if n > 0 {
        let tonight = world.day() + 1;
        let mut cands: Vec<(bool, usize, EntityId)> = free
            .iter()
            .enumerate()
            .filter(|&(_, &m)| in_reach(world, c, m, hideout))
            .map(|(i, &m)| (!crate::systems::gang::on_watch_night(world, m, tonight), i, m))
            .collect();
        cands.sort_unstable();
        cands.truncate(n);
        for &(_, _, m) in &cands {
            free.retain(|&x| x != m);
            post(world, c, m, gang, DutyKind::Guard, hideout, c.guard_hour);
        }
        ownership::jobs2_row(world).seat_guards_posted += cands.len() as u32;
    }
    // The order's duty.
    let window_end = world.tick + TICKS_PER_DAY;
    let raid_today = order.is_raid() && raid_at.is_some_and(|t| t < window_end);
    if raid_today {
        let hour = raid_at.map_or(c.guard_hour, |t| ((t % TICKS_PER_DAY) / TICKS_PER_HOUR) as u8);
        // Pressure pass round 3, S1 (OPEN-221): the leader too says so when
        // he leaves the march to his crew.
        if let Some(l) = leader.filter(|&l| free_for_duty(world, l)) {
            muster_reach(world, c, l, gang);
        }
        for m in std::mem::take(&mut free) {
            if muster_reach(world, c, m, gang) {
                post(world, c, m, gang, DutyKind::Muster, muster_at(world, gang, hideout), hour);
            }
        }
    } else {
        let kind = match order {
            Order::Contest | Order::Retaliate => Some(DutyKind::Guard),
            Order::Expand | Order::Squat | Order::Harvest | Order::VirtRaid | Order::Job => Some(DutyKind::Watch),
            _ => None,
        };
        if let Some(kind) = kind {
            let slots = paid_slots(world, c, gang, kind, duty_slots(c, members.len()));
            let mut n = 0;
            let mut i = 0;
            while i < free.len() && n < slots {
                let m = free[i];
                let place = if kind == DutyKind::Guard { Some(hideout) } else { watch_place(world, c, gang, m) };
                match place.filter(|&p| in_reach(world, c, m, p)) {
                    Some(p) => {
                        free.remove(i);
                        let hour = if kind == DutyKind::Guard { c.guard_hour } else { c.watch_hour };
                        post(world, c, m, gang, kind, p, hour);
                        n += 1;
                    }
                    None => i += 1,
                }
            }
        }
    }
    // Rest days: the crew.
    for m in free {
        if works_today(world, m) || hash(world, PURPOSE_CREW, m, day) >= c.crew_p {
            continue;
        }
        let Some(p) = crew_place(world, c, gang, m) else { continue };
        post(world, c, m, gang, DutyKind::Crew, p, c.crew_hour);
    }
}

/// The city milestone C9b: `m` stands today's guard at his gang's seat (a
/// Guard duty there he did not trade for the shift): he stays out of the raid.
pub fn seat_guard_today(world: &World, m: EntityId) -> bool {
    let (today, small_hours) = (world.day(), world.tick_of_day() < 6 * TICKS_PER_HOUR as u16);
    world.crew.duties.get(&m).is_some_and(|d| {
        d.kind == DutyKind::Guard
            && !d.chose_job
            && (d.day == today || (small_hours && d.day + 1 == today))
            && world.gang_of(m) == Some(d.gang)
            && world.hideout_of(d.gang) == Some(d.place)
            && crate::systems::vertical::seat_guard_n(world) > 0
    })
}

/// R5 (OPEN-214): a raid ordered after midnight's duties were posted, its
/// muster before the next midnight: the members free and holding no duty
/// today are called to it now (V10: a Raid ordered at 06:58 scored ~0.3 on
/// courage and loyalty alone, with no muster posted).
pub fn raid_ordered(world: &mut World, gang: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (leader, raid_at, members) = (g.leader, g.raid_at, g.members.clone());
    let Some(at) = raid_at.filter(|_| g.order.is_raid()) else { return };
    let midnight = (world.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    if at >= midnight {
        return;
    }
    let Some(hideout) = world.hideout_of(gang) else { return };
    let hour = ((at % TICKS_PER_DAY) / TICKS_PER_HOUR) as u8;
    let today = world.day();
    for m in members {
        if Some(m) == leader || !free_for_duty(world, m) {
            continue;
        }
        if world.crew.duties.get(&m).is_some_and(|d| d.day == today) {
            continue;
        }
        if !muster_reach(world, &c, m, gang) {
            continue;
        }
        post(world, &c, m, gang, DutyKind::Muster, muster_at(world, gang, hideout), hour);
    }
}

/// R5 (OPEN-214): the watch or guard slots a gang can pay today (its
/// treasury over `pay_reserve`, `duty_pay` a slot), at most `slots`. A gang
/// that can pay none stands its duty down (told once, and again when it
/// pays): V10's "unpaid: the gang is broke" every night on the same watch.
fn paid_slots(world: &mut World, c: &CrewCfg, gang: EntityId, kind: DutyKind, slots: usize) -> usize {
    let spare = (world.purse(Some(gang)) - c.pay_reserve).max(0);
    let can = if c.duty_pay > 0 { usize::try_from(spare / c.duty_pay).unwrap_or(usize::MAX) } else { slots };
    let n = slots.min(can);
    let today = world.day();
    let was = world.crew.stood_down.contains_key(&gang);
    let gname = gang_name(world, gang);
    let mut actors = vec![gang];
    actors.extend(world.comp::<Gang>(gang).and_then(|g| g.leader));
    if n == 0 && slots > 0 && !was {
        world.crew.stood_down.insert(gang, today);
        let text = format!(
            "{gname} stood its {} down: the treasury ({}c) cannot pay a {}c duty",
            kind.label(),
            world.purse(Some(gang)),
            c.duty_pay
        );
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_stood_down += 1;
    } else if n > 0 && was {
        world.crew.stood_down.remove(&gang);
        let text = format!("{gname} can pay a {} again: {n} posted", kind.label());
        world.push_event(EventKind::Crew, &actors, text);
    }
    n
}

/// R5 (OPEN-214): the Raid goal's duty flat: `muster_flat` for a member
/// posted to today's muster who did not choose the shift; 0 otherwise.
pub fn muster_flat(world: &World, id: EntityId) -> f32 {
    let Some(c) = cfg(world) else { return 0.0 };
    match world.crew.duties.get(&id) {
        Some(d) if d.kind == DutyKind::Muster && !d.chose_job && world.gang_of(id) == Some(d.gang) => c.muster_flat,
        _ => 0.0,
    }
}

/// R5 (OPEN-214): `plan` holds against anything but danger or starving
/// (`fixes::committed`): a recruit's try-out (the walk there and the half
/// hour; V10: a 254-min walk, then Sleep at the door 14 minutes in), and a
/// raid's muster until the march (V10: both musters cut by Sleep, 0
/// brawls), unless the member chose the shift over it at midnight. A try-out
/// gives way to the shift (`winner` Work), a raid only to the chosen one.
pub fn holds(world: &World, id: EntityId, plan: &crate::goap::Plan, winner: crate::components::GoalKind) -> bool {
    if !on(world) {
        return false;
    }
    match plan.goal {
        crate::components::GoalKind::JoinGang => {
            !world.has::<GangMember>(id) && winner != crate::components::GoalKind::Work
        }
        crate::components::GoalKind::Raid => {
            crate::systems::raid::raid_pending(world, id)
                && !crate::systems::raid::raid_done(world, id)
                && !world.crew.duties.get(&id).is_some_and(|d| d.kind == DutyKind::Muster && d.chose_job)
        }
        _ => false,
    }
}

/// R5 (OPEN-214): a full door does not keep the crew out: a member into his
/// own gang's Hideout, a member to today's duty place, a recruit to his
/// try-out. The Hideout's capacity is its beds; V10's musters, courts and
/// rest nights at a full Hideout failed `BuildingFull` at the door.
pub fn capacity_exempt(world: &World, agent: EntityId, b: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    if let Some(g) = world.gang_of(agent) {
        if world.hideout_of(g) == Some(b) || world.crew.duties.get(&agent).is_some_and(|d| d.place == b) {
            return true;
        }
    }
    world
        .comp::<crate::components::Brain>(agent)
        .and_then(|br| br.plan.as_ref())
        .is_some_and(|p| p.goal == crate::components::GoalKind::JoinGang && p.target == Some(b))
}

/// A crew Visit arrived (`household::credit`), or a Statistical one at its
/// hour: the duty kept, the pay, the evening's company, the welcome.
pub fn arrived(world: &mut World, id: EntityId, place: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(d) = world.crew.duties.get(&id).cloned() else { return };
    if d.place != place || d.kept || world.gang_of(id) != Some(d.gang) {
        return;
    }
    if let Some(x) = world.crew.duties.get_mut(&id) {
        x.kept = true;
    }
    let gname = gang_name(world, d.gang);
    let pname = world.name_of(place);
    let crew = company(world, id, d.gang, place);
    let names = names_of(world, &crew);
    match d.kind {
        DutyKind::Watch | DutyKind::Guard => {
            let pay = pay(world, &c, id, d.gang);
            let verb = if d.kind == DutyKind::Watch { "stood watch at" } else { "stood guard at" };
            let with = if crew.is_empty() { String::new() } else { format!(" with {names}") };
            let text = format!("{} {verb} {pname} for {gname}{with} ({pay})", world.name_of(id));
            world.push_event(EventKind::Crew, &[id, d.gang, place], text);
            ownership::jobs2_row(world).crew_duty_kept += 1;
        }
        DutyKind::Court => {
            let today = world.day() as u32;
            let over = world.crew.failed_courts.get(&d.gang).is_some_and(|v| v.contains(&today));
            let text = if over {
                format!("{} came to {gname}'s court at {pname} too late: it was over", world.name_of(id))
            } else {
                format!("{} answered {gname}'s summons to court at {pname}", world.name_of(id))
            };
            world.push_event(EventKind::Crew, &[id, d.gang, place], text);
            ownership::jobs2_row(world).crew_duty_kept += 1;
        }
        DutyKind::Crew => {
            let text = if crew.is_empty() {
                format!("{} went to the crew at {pname} on a day off; nobody else came", world.name_of(id))
            } else {
                format!("{} spent a day off with the crew at {pname}: {names} there", world.name_of(id))
            };
            for &o in &crew {
                crate::systems::social::interacted(world, id, o);
            }
            if !crew.is_empty() {
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.belonging = (n.belonging + 0.3).min(1.0);
                }
            }
            let mut actors = vec![id, d.gang, place];
            actors.extend(crew.iter().copied());
            world.push_event(EventKind::Crew, &actors, text);
            ownership::jobs2_row(world).crew_rest_nights += 1;
        }
        DutyKind::Welcome => welcome(world, &c, id, d.gang, place, &crew, walked),
        DutyKind::MoveIn => {
            // The Hideout's storey (an off-screen body is credited at the door).
            let floor = world.comp::<Building>(place).and_then(|b| b.span).map_or(0, |(lo, _)| lo);
            let host = crate::systems::floors::host_of(world, place);
            let with = if crew.is_empty() { String::new() } else { format!(" ({names} there)") };
            let text = format!(
                "{} walked into {gname}'s new Hideout {pname} on floor {floor} of {}{with}",
                world.name_of(id),
                world.name_of(host)
            );
            let mut actors = vec![id, d.gang, place];
            actors.extend(crew.iter().copied());
            world.push_event(EventKind::Crew, &actors, text);
            ownership::jobs2_row(world).found_walkins += 1;
        }
        DutyKind::Muster => {}
    }
}

/// The gang-mates at `place` now: bodies inside; with none, the members
/// whose own duty there today was kept (the off-screen crew).
fn company(world: &World, id: EntityId, gang: EntityId, place: EntityId) -> Vec<EntityId> {
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    members
        .into_iter()
        .filter(|&m| m != id)
        .filter(|&m| {
            presence::body_at(world, m, place) || world.crew.duties.get(&m).is_some_and(|d| d.place == place && d.kept)
        })
        .take(4)
        .collect()
}

pub(super) fn names_of(world: &World, ids: &[EntityId]) -> String {
    ids.iter().map(|&x| world.name_of(x)).collect::<Vec<_>>().join(", ")
}

/// The duty's pay, from the treasury above `pay_reserve`, less the bail
/// owed. Returns the line's words.
pub(super) fn pay(world: &mut World, c: &CrewCfg, id: EntityId, gang: EntityId) -> String {
    let spare = world.purse(Some(gang)) - c.pay_reserve;
    if spare < c.duty_pay || c.duty_pay <= 0 {
        return "unpaid: the gang is broke".to_string();
    }
    let owed = world.crew.books.get(&id).map_or(0, |b| b.owed);
    let kept = owed.min(c.duty_pay);
    if kept > 0 {
        if let Some(b) = world.crew.books.get_mut(&id) {
            b.owed -= kept;
        }
    }
    let paid = ownership::pay(world, Some(gang), Some(id), c.duty_pay - kept, Flow::Tribute);
    ownership::jobs2_row(world).crew_duty_pay += paid;
    crate::systems::gang::note_paid(world, id);
    if kept > 0 {
        format!("paid {paid}c, {kept}c kept against his bail")
    } else {
        format!("paid {paid}c")
    }
}

/// Round 3 S2 (OPEN-222): a member who sat the muster to its departure
/// (`raid::depart`) kept it: a march cut short on the way (Sleep, a
/// street cut) is no miss (`raider_39544` mustered 120 min and was marked
/// "missed" when only the brawl's present counted).
pub fn sat_muster(world: &mut World, gang: EntityId, m: EntityId) {
    if !on(world) {
        return;
    }
    let (muster, today) = (world.hideout_of(gang), world.day());
    let mut turned = false;
    if let Some(d) = world.crew.duties.get_mut(&m) {
        if d.gang == gang && d.kind == DutyKind::Muster {
            d.kept = true;
        } else if d.gang == gang && d.day == today && !d.kept && d.kind.owed() {
            // Round 4 T3 (OPEN-222): the gang's raid outranks the day's other
            // owed duty (a court summons, a watch, a guard): he answered the
            // muster, so the duty he held becomes it, kept (V11's and V12's
            // `raider_70326` mustered 353 min at the Hideout he was summoned
            // to, marched, and was "missed ... court summons" at midnight:
            // a raid ordered after midnight posts no muster over a duty).
            d.kind = DutyKind::Muster;
            d.place = muster.unwrap_or(d.place);
            d.kept = true;
            turned = true;
        }
    }
    if turned && world.visits.get(&m).is_some_and(|v| v.why == VisitWhy::Crew) {
        world.visits.remove(&m);
    }
}

/// The raid's muster kept by those at the door (`raid::resolve`).
pub fn mustered(world: &mut World, gang: EntityId, present: &[EntityId]) {
    if !on(world) {
        return;
    }
    let today = world.day();
    world.crew.raided_day.insert(gang, today);
    for &m in present {
        if let Some(d) = world.crew.duties.get_mut(&m) {
            if d.gang == gang && d.kind == DutyKind::Muster {
                d.kept = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Raids write a result
// ---------------------------------------------------------------------------

/// What a raid's brawl left (`raid::brawl`, `raid::corp_brawl`, `raid::breach`).
pub struct RaidLeft<'a> {
    pub gang: EntityId,
    /// The defending gang (a rival Hideout), `None` at a corp's door.
    pub rival: Option<EntityId>,
    pub place: &'a str,
    pub raiders: &'a [EntityId],
    pub defenders: &'a [EntityId],
    pub raiders_standing: &'a [EntityId],
    pub defenders_standing: &'a [EntityId],
    pub won: bool,
    pub coins: i64,
    pub food: u32,
    /// Round 3 S7 (OPEN-225): a breakout at the Precinct (`raid::breach`), the
    /// convicts it freed: the line reads them, and the guards beaten hold no
    /// crew grudge (the law answers a breakout through `law::on_breakout`).
    pub freed: Option<&'a [EntityId]>,
}

/// `ids` named up to `[noise] raid_names`, the rest "and N more".
fn names_capped(world: &World, ids: &[EntityId]) -> String {
    names_with(world, ids, |x| world.name_of(x))
}

/// V8 (`OPEN-264`): [`names_capped`] with each name's age ("Name (34 y)"), for
/// the raid's hurt and dead (the under-18 rule read off the line).
fn names_aged(world: &World, ids: &[EntityId]) -> String {
    names_with(world, ids, |x| {
        let age = world.comp::<crate::components::Identity>(x).map_or(0, |i| i.age_years());
        format!("{} ({age} y)", world.name_of(x))
    })
}

fn names_with(world: &World, ids: &[EntityId], name: impl Fn(EntityId) -> String) -> String {
    let cap = usize::from(world.config.noise.raid_names);
    let shown: Vec<String> = ids.iter().take(cap).map(|&x| name(x)).collect();
    let more = ids.len().saturating_sub(shown.len());
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    format!("{}{tail}", shown.join(", "))
}

/// The raid's result line for every raider and defender, the toll counted
/// (round 3 S7: the dead and the hurt by count, the first `[noise] raid_names`
/// named), the crews' grudges, the repelled raid answered, the next orders.
pub fn raid_result(world: &mut World, r: RaidLeft) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    let gname = gang_name(world, r.gang);
    let rname = r.rival.map_or_else(|| r.place.to_string(), |x| gang_name(world, x));
    let beaten_def: Vec<EntityId> = r.defenders.iter().copied().filter(|d| !r.defenders_standing.contains(d)).collect();
    let beaten_raid: Vec<EntityId> = r.raiders.iter().copied().filter(|x| !r.raiders_standing.contains(x)).collect();
    let mut wounded = Vec::new();
    let mut dead = Vec::new();
    for &x in beaten_def.iter().chain(beaten_raid.iter()) {
        if !world.has::<crate::components::Brain>(x) {
            continue;
        }
        if law::living(world, x) {
            wounded.push(x);
        } else {
            dead.push(x);
        }
    }
    // The crews' grudges: the beaten on the other gang, the losing leader
    // on the winner.
    for &d in beaten_def.iter().filter(|_| r.freed.is_none()) {
        if law::living(world, d) {
            crate::systems::grudges::add(world, d, r.gang, crate::word::GrudgeCause::Assaulted, c.raid_grudge, 0);
        }
    }
    if let Some(rival) = r.rival {
        for &x in &beaten_raid {
            if law::living(world, x) {
                crate::systems::grudges::add(world, x, rival, crate::word::GrudgeCause::Assaulted, c.raid_grudge, 0);
            }
        }
        let (loser, winner) = if r.won { (rival, r.gang) } else { (r.gang, rival) };
        if let Some(l) = world.comp::<Gang>(loser).and_then(|g| g.leader) {
            crate::systems::grudges::add(
                world,
                l,
                winner,
                crate::word::GrudgeCause::Assaulted,
                c.raid_leader_grudge,
                0,
            );
        }
        // A repelled raid is answered.
        if !r.won {
            if let Some(g) = world.comp_mut::<Gang>(rival) {
                g.retaliate_until = Some(now + c.repelled_retaliate_days * TICKS_PER_DAY);
                if g.order != Order::Retaliate {
                    g.retaliate_on = Some(r.gang);
                }
            }
            crate::systems::gang::push_shock(world, rival, Shock::Raided);
            crate::systems::faction::rethink(world, rival);
        }
    }
    // The wounded: the lost fight is in their memory (phase 2's care reads
    // it); the line names where the gang-hurt go.
    let wounded_text = if wounded.is_empty() {
        String::new()
    } else {
        format!("; {} hurt: {}", wounded.len(), names_aged(world, &wounded))
    };
    let dead_text =
        if dead.is_empty() { String::new() } else { format!("; {} dead: {}", dead.len(), names_aged(world, &dead)) };
    let took = match (r.won, r.rival.is_some(), r.freed) {
        (true, _, Some([])) => "; nobody inside to free".to_string(),
        (true, _, Some(f)) => format!("; {} freed: {}", f.len(), names_capped(world, f)),
        (false, _, Some(_)) => "; nobody freed".to_string(),
        (true, true, None) => format!("; {gname} took {}c and {} food from the Hideout", r.coins, r.food),
        (true, false, None) => format!("; the till smashed, {}c and {} food taken", r.coins, r.food),
        (false, _, None) => "; nothing taken".to_string(),
    };
    let next = |w: &World, g: EntityId| {
        w.comp::<Gang>(g).map_or_else(String::new, |x| {
            let o = x.pending_order.as_ref().map_or(x.order, |p| p.order);
            format!("{} next: {o:?}", x.name)
        })
    };
    let mut nexts = vec![next(world, r.gang)];
    if let Some(rv) = r.rival {
        nexts.push(next(world, rv));
    }
    // The city milestone C9b: a door nobody held is a walk-in, not a win.
    let verdict = match (r.won, r.defenders.is_empty(), r.freed.is_some()) {
        (true, true, false) => format!("{gname} walked in, nobody held it"),
        (true, false, false) if beaten_raid.is_empty() => format!("{gname} sacked it"),
        (true, _, _) => format!("{gname} won"),
        (false, _, _) => format!("{rname} held"),
    };
    let text = format!(
        "Raid on {}: {verdict} ({} raiders, {} defenders){took}{wounded_text}{dead_text}; {}",
        r.place,
        r.raiders.len(),
        r.defenders.len(),
        nexts.join(", ")
    );
    let mut actors: Vec<EntityId> = r.raiders.iter().chain(r.defenders.iter()).copied().collect();
    actors.sort_unstable();
    actors.dedup();
    let fought = actors.clone();
    actors.push(r.gang);
    actors.extend(r.rival);
    world.push_event(EventKind::Raid, &actors, text);
    let summary =
        if r.won { format!("{gname} won{took}{wounded_text}") } else { format!("{rname} held{wounded_text}") };
    world.crew.last_raid.insert(r.gang, (now, format!("{} ({summary})", r.place), fought));
    let row = ownership::jobs2_row(world);
    row.crew_raid_results += 1;
    row.crew_raid_wounded += wounded.len() as u32;
}

/// A marcher at the door after the brawl was resolved (`raid::resolve`
/// found no raid standing): the result as he finds it, within two hours.
pub fn late_to_raid(world: &mut World, actor: EntityId, gang: EntityId) {
    if !on(world) {
        return;
    }
    let Some((t, what, fought)) = world.crew.last_raid.get(&gang).cloned() else { return };
    if world.tick.saturating_sub(t) > 2 * TICKS_PER_HOUR || fought.binary_search(&actor).is_ok() {
        return;
    }
    let text = format!("{} reached {what} after it was over", world.name_of(actor));
    world.push_event(EventKind::Raid, &[actor, gang], text);
}

/// Round 7 W2 (`OPEN-241`): `id` was still on the way to his gang's muster
/// or its door when the raid was over (fought without him, called off,
/// fizzled): the walk is released (`think::apply`).
pub fn march_released(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    let recent = world.crew.last_raid.get(&gang).filter(|x| world.tick.saturating_sub(x.0) <= 2 * TICKS_PER_HOUR);
    // In the brawl from within reach of the door: not missed.
    if recent.is_some_and(|x| x.2.contains(&id)) {
        return;
    }
    crate::systems::ownership::jobs2_row(world).raid_missed += 1;
    if !on(world) {
        return;
    }
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let name = world.name_of(id);
    let text = match world.crew.last_raid.get(&gang) {
        Some((t, what, _)) if world.tick.saturating_sub(*t) <= 2 * TICKS_PER_HOUR => {
            format!("{name} was still on the way when {gname} went in at {what}, and turned back")
        }
        _ => format!("{name} was still on the way when {gname}'s raid was off, and turned back"),
    };
    world.push_event(EventKind::Raid, &[id, gang], text);
}

// ---------------------------------------------------------------------------
// Court's quorum and the gang that cannot muster
// ---------------------------------------------------------------------------

/// Court ended at the Hideout (`leaders::court_done`): the summoned there
/// make the quorum, else the court fails with the reasons.
pub fn court_held(world: &mut World, gang: EntityId, leader: EntityId) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(hideout) = world.hideout_of(gang) else { return };
    let today = world.day();
    let summoned: Vec<(EntityId, Duty)> = world
        .crew
        .duties
        .iter()
        .filter(|(_, d)| d.gang == gang && d.kind == DutyKind::Court && d.day == today)
        .map(|(&m, d)| (m, d.clone()))
        .collect();
    if summoned.is_empty() {
        return;
    }
    let mut came = Vec::new();
    let mut reasons = Vec::new();
    for (m, d) in &summoned {
        if d.kept || presence::body_at(world, *m, hideout) {
            came.push(*m);
            if let Some(x) = world.crew.duties.get_mut(m) {
                x.kept = true;
            }
        } else {
            let why = if world.has::<Sentence>(*m) {
                "in the cells"
            } else if d.chose_job {
                "at work"
            } else if world.gang_of(*m) != Some(gang) {
                "gone"
            } else {
                "did not come"
            };
            reasons.push(format!("{} ({why})", world.name_of(*m)));
        }
    }
    let lname = world.name_of(leader);
    if came.len() < usize::from(c.court_quorum) {
        let fails = {
            let day = today as u32;
            let w = c.fold_window_days;
            let v = world.crew.failed_courts.entry(gang).or_default();
            v.retain(|&x| x + w > day);
            v.push(day);
            v.len()
        };
        let text = format!(
            "{lname}'s court at {} failed: nobody came (summoned {}; {fails} failed in {} days)",
            world.name_of(hideout),
            reasons.join(", "),
            c.fold_window_days
        );
        world.push_event(EventKind::Crew, &[leader, gang, hideout], text);
        ownership::jobs2_row(world).crew_court_failed += 1;
    } else {
        let absent = if reasons.is_empty() { String::new() } else { format!("; absent {}", reasons.join(", ")) };
        let text = format!("{lname} held court at {}: {} came{absent}", world.name_of(hideout), names_of(world, &came));
        let mut actors = vec![leader, gang, hideout];
        actors.extend(came.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_court_quorum += 1;
    }
}

/// A gang with `fold_fails` failed courts in the window takes a rival's
/// offer or folds.
pub(super) fn fold_check(world: &mut World, c: &CrewCfg, gang: EntityId, day: u32) {
    let fails =
        world.crew.failed_courts.get(&gang).map_or(0, |v| v.iter().filter(|&&x| x + c.fold_window_days > day).count());
    if (fails as u32) < c.fold_fails.max(1) {
        return;
    }
    world.crew.failed_courts.remove(&gang);
    let Some(g) = world.comp::<Gang>(gang) else { return };
    if g.members.is_empty() {
        return;
    }
    let (leader, creed, members) = (g.leader, g.creed, g.members.clone());
    let pride = leader.and_then(|l| world.comp::<Personality>(l)).map_or(0.5, |p| p.pride);
    let buyer = if creed.is_some() { None } else { absorber(world, gang) };
    let wa = buyer.map_or(0.0, |_| c.w_absorb * (1.5 - pride).max(0.0));
    let wf = c.w_fold * (0.5 + pride);
    let u = hash(world, PURPOSE_FOLD, gang, u64::from(day)) * (wa + wf);
    let gname = gang_name(world, gang);
    if let (Some(b), true) = (buyer, u < wa) {
        let bname = gang_name(world, b);
        let offer = leader.map_or(0, |l| ownership::pay(world, Some(b), Some(l), c.absorb_offer, Flow::Tribute));
        let chest = world.purse(Some(gang)).max(0);
        let moved = ownership::pay(world, Some(gang), Some(b), chest, Flow::Tribute);
        let reason = format!("absorbed into {bname}");
        // The leader last: his colours change with the rest.
        let mut order: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
        order.extend(leader);
        for m in &order {
            crate::systems::gang::leave(world, *m, &reason);
            if law::living(world, *m) {
                crate::systems::gang::enlist(world, *m, b);
            }
        }
        let lname = leader.map_or_else(|| "nobody".to_string(), |l| world.name_of(l));
        let text = format!(
            "{gname} could not muster and took {bname}'s offer: {} members and {moved}c went over, {offer}c to {lname}",
            order.len()
        );
        let mut actors = vec![gang, b];
        actors.extend(order.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_absorbed += 1;
    } else {
        let chest = world.purse(Some(gang)).max(0);
        let each = if members.is_empty() { 0 } else { chest / members.len() as i64 };
        if each > 0 {
            for &m in &members {
                ownership::pay(world, Some(gang), Some(m), each, Flow::Tribute);
            }
        }
        let mut order: Vec<EntityId> = members.iter().copied().filter(|&m| Some(m) != leader).collect();
        order.extend(leader);
        for m in &order {
            crate::systems::gang::leave(world, *m, "folded");
        }
        let text = format!("{gname} could not muster and folded: {} members walk, {each}c each", order.len());
        let mut actors = vec![gang];
        actors.extend(order.iter().copied());
        world.push_event(EventKind::Crew, &actors, text);
        ownership::jobs2_row(world).crew_folded += 1;
    }
}
