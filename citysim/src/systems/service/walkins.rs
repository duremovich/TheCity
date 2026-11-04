use super::*;

/// § 2.4 (fixer, `contracts::post_with` and `accept`): a brokered post or
/// take is a `Client` encounter with the fixer on duty (`contract_encounters`):
/// a body inside the Fixer meets him there, a party elsewhere (an order, a
/// coerced post, a Statistical take) is credited world-side at the hour, to
/// the fixer on duty or, off the open hours (the corps post at midnight,
/// the takes come at the match hour), to the Fixer's book ([`fixer_book`]).
/// Tower only.
pub fn contract_party(world: &mut World, party: EntityId, fixer: EntityId) {
    contract_party_at(world, party, fixer, true);
}

/// The pressure pass phase 3 (OPEN-157): a contract's party sent to the
/// Fixer in its open hours (`VisitWhy::Broker`) is there: the encounter now
/// if a server is on duty (else it is missed, not sent again).
pub fn broker_arrived(world: &mut World, party: EntityId, fixer: EntityId) {
    ownership::jobs2_row(world).broker_walks += 1;
    contract_party_at(world, party, fixer, false);
}

/// OPEN-157: send `party` to `fixer` in person at its next open hour today
/// (the staff's first shift hour, or the next hour if later), when the
/// visits are on and the party has no other visit due. `false`: not sent.
fn send_broker(world: &mut World, party: EntityId, fixer: EntityId) -> bool {
    if !world.config.protection.as_ref().is_some_and(|c| c.broker_visits)
        || !crate::systems::household::on(world)
        || world.visits.contains_key(&party)
        || world.comp::<Building>(fixer).is_none_or(|b| b.demolished || b.derelict)
    {
        return false;
    }
    let Some(open) = fixer_first_hour(world, fixer) else { return false };
    let now = u8::try_from(u64::from(world.tick_of_day()) / TICKS_PER_HOUR).unwrap_or(23);
    let hour = open.max(now.saturating_add(1));
    if hour >= 23 {
        return false;
    }
    let v =
        crate::systems::household::PendingVisit { kin: fixer, why: crate::systems::household::VisitWhy::Broker, hour };
    world.visits.insert(party, v);
    true
}

fn contract_party_at(world: &mut World, party: EntityId, fixer: EntityId, may_send: bool) {
    if !tower_on(world) || party == EntityId::NONE || !world.has::<Brain>(party) {
        return;
    }
    // The pre-view pass phase 4 (P3): the encounter needs both parties: a
    // server on duty in the Fixer and a body party inside it (a Statistical
    // one is credited at the hour); the book credit off the hours only
    // between two Statistical parties (the mirror), never to a body asleep.
    if crate::systems::presence::on(world) {
        let body = |x: EntityId| crate::systems::presence::is_body(world, x);
        let here = !body(party) || crate::systems::presence::body_at(world, party, fixer);
        let server = fixer_server(world, fixer, party)
            .filter(|_| here)
            .or_else(|| fixer_book(world, fixer, party).filter(|&s| !body(s) && !body(party)));
        let Some(server) = server else {
            // The pressure pass phase 3 (OPEN-157): the party comes in
            // person in the open hours (both tiers: a Statistical party is
            // credited at the visit's hour).
            if may_send && send_broker(world, party, fixer) {
                return;
            }
            crate::systems::presence::missed(world);
            return;
        };
        ownership::jobs2_row(world).contract_encounters += 1;
        fixer_meet(world, server, party, fixer, 0);
        return;
    }
    let Some(server) = fixer_server(world, fixer, party).or_else(|| fixer_book(world, fixer, party)) else { return };
    ownership::jobs2_row(world).contract_encounters += 1;
    fixer_meet(world, server, party, fixer, 0);
}

/// Is the walk-in's buyer walking to the Fixer (a body with the `Papers`
/// visit pending)? Its sale is made at the arrival ([`walk_in_arrived`]).
fn walking_in(world: &World, w: &WalkIn) -> bool {
    world.comp::<Brain>(w.buyer).is_some_and(|b| b.lod != Lod::Statistical)
        && world
            .visits
            .get(&w.buyer)
            .is_some_and(|v| v.kin == w.fixer && v.why == crate::systems::household::VisitWhy::Papers)
}

/// The walk-in's pickup, if a server is on duty: the order was paid at
/// midnight (`gray::papers_daily`: the coins and `fake_sold` as main's, so
/// the money circuit moves at main's tick); now `Papers` are inserted (their
/// protection starts here), an encounter with the server and a `Served`
/// event. `Some(true)` handed over, `Some(false)` nobody on duty (it
/// waits), `None` the pickup is off (the buyer gone, jailed or papered, the
/// Fixer gone: `walk_ins_dropped`; the forger keeps the coins).
/// Round 5 U5 (OPEN-217): the forger who made it hands it over when he is
/// behind the counter with room this hour (at 20k the counter's other
/// server took the sale and the forger was paid asleep at home,
/// `forger_19041 d21 13:01 SETTLED APART`); else the counter's server
/// hands over his work. A forged licence is the same walk-in at `[gray]
/// licence_price`.
fn walk_in_sale(world: &mut World, w: WalkIn) -> Option<bool> {
    let has = match w.kind {
        WalkKind::Papers => world.jobs2.gray.papers.contains_key(&w.buyer),
        WalkKind::Licence => crate::systems::weapons::licensed(world, w.buyer),
    };
    if !crate::systems::law::living(world, w.buyer)
        || world.has::<Sentence>(w.buyer)
        || has
        || world.comp::<Building>(w.fixer).is_none_or(|b| b.demolished || b.derelict)
    {
        ownership::jobs2_row(world).walk_ins_dropped += 1;
        return None;
    }
    // The pre-view pass phase 4 (P3): a body buyer is handed his papers in
    // the Fixer, not across the city (he waits for his walk).
    if crate::systems::presence::on(world)
        && crate::systems::presence::is_body(world, w.buyer)
        && !crate::systems::presence::body_at(world, w.buyer, w.fixer)
    {
        return Some(false);
    }
    let alive = crate::systems::law::living(world, w.forger);
    let at_desk = alive && w.forger != w.buyer && present(world, w.forger, w.fixer) && has_room(world, w.forger);
    // The forger due at his desk later today: an order of today waits for
    // him (100k: the counter opened at 09:00, the commuting forger came in
    // at 09:2x, and the hired Fixer handed over his work, `fixer_98933 d22
    // 09:00` "(Moth Bellamy made)"); from the next day the counter hands it.
    if !at_desk && w.day == world.day() && forger_due_today(world, w.forger, w.fixer) {
        return Some(false);
    }
    let server = if at_desk {
        w.forger
    } else {
        match fixer_server(world, w.fixer, w.buyer) {
            Some(s) => s,
            None => return Some(false),
        }
    };
    let tier = w.tier.max(1);
    let price = match w.kind {
        WalkKind::Papers => world.config.gray.id_price[usize::from(tier - 1)],
        WalkKind::Licence => world.config.gray.licence_price,
    };
    // Round 5 U5: a forger in the cells (or dead) made nothing today: the
    // counter's server is paid as the maker (20k: `forger_9672 d21 12:08`,
    // his share paid to him in the cells).
    let forger = if alive && !world.has::<Sentence>(w.forger) { w.forger } else { server };
    // The pre-view pass phase 4 (P3, OPEN-119): paid here, at the handover:
    // the forger's share to him, the owner's cut to the server, who holds it
    // for the owner's collection in person (`collected`).
    let mut held = 0;
    if w.unpaid {
        if world.purse(Some(w.buyer)) < price {
            ownership::jobs2_row(world).walk_ins_dropped += 1;
            return None;
        }
        let cut = (price as f32 * world.config.gray.fixer_cut).round() as i64;
        ownership::pay(world, Some(w.buyer), Some(forger), price - cut, Flow::Gray);
        held = ownership::pay(world, Some(w.buyer), Some(server), cut, Flow::Gray);
        if held > 0 {
            world.fixer_owed.entry(w.fixer).or_default().push((server, held));
        }
        if w.kind == WalkKind::Papers {
            let row = ownership::jobs2_row(world);
            row.fake_sold += 1;
            row.papers_handover += 1;
        }
    }
    let what = match w.kind {
        WalkKind::Papers => {
            let p = crate::systems::gray::Papers { tier, fake: true, forger: Some(forger) };
            world.jobs2.gray.papers.insert(w.buyer, p);
            "papers"
        }
        WalkKind::Licence => {
            world.jobs2.gray.licences.insert(w.buyer, forger);
            ownership::jobs2_row(world).licences_forged += 1;
            "a forged weapons licence"
        }
    };
    ownership::jobs2_row(world).walk_ins_served += 1;
    fixer_meet(world, server, w.buyer, w.fixer, price);
    let made = if forger == server { String::new() } else { format!(" ({} made)", world.name_of(forger)) };
    let paid = match (w.unpaid, held > 0) {
        (true, true) => format!(", {price}c paid, {held}c of it held for the owner"),
        (true, false) => format!(", {price}c paid"),
        (false, _) => String::new(),
    };
    let text = format!(
        "{} handed {} {what}{made} at {}{paid}",
        world.name_of(server),
        world.name_of(w.buyer),
        world.name_of(w.fixer)
    );
    let mut actors = vec![server, w.buyer, w.fixer];
    if forger != server {
        actors.push(forger);
    }
    world.push_event(EventKind::Served, &actors, text);
    Some(true)
}

/// Round 5 U5: `forger` alive, free, on the books at `fixer` and on a
/// workday with his shift not yet over today.
fn forger_due_today(world: &World, forger: EntityId, fixer: EntityId) -> bool {
    if !crate::systems::law::living(world, forger) || world.has::<Sentence>(forger) {
        return false;
    }
    let tod = world.tick_of_day();
    world.comp::<Job>(forger).is_some_and(|j| {
        j.employer == Some(fixer)
            && j.shifts.iter().any(|&(_, end)| tod < end)
            && crate::exec::routine::workday_of(world, forger, j, j.shift_key_at(world.tick))
    })
}

/// § 2.4 (fixer): each hour, the walk-ins not walking are served by the
/// Fixer's server on duty (a Statistical buyer credited at the hour a
/// body's walk would land in: the Fixer's open hours); unserved, they wait.
pub(super) fn walk_in_hour(world: &mut World) {
    if world.walk_ins.is_empty() {
        return;
    }
    let pending = std::mem::take(&mut world.walk_ins);
    let mut keep = Vec::with_capacity(pending.len());
    for w in pending {
        if walking_in(world, &w) {
            keep.push(w);
            continue;
        }
        if walk_in_sale(world, w) == Some(false) {
            keep.push(w);
        }
    }
    keep.append(&mut world.walk_ins);
    world.walk_ins = keep;
}

/// § 2.4 (fixer, `household::credit`): the walking buyer is at the Fixer:
/// the sale now, if a server is on duty (else the walk-in waits for the
/// hour's pass, the walk spent).
pub fn walk_in_arrived(world: &mut World, buyer: EntityId, fixer: EntityId) {
    let Some(w) = world.walk_ins.iter().copied().find(|w| w.buyer == buyer && w.fixer == fixer) else { return };
    if walk_in_sale(world, w) != Some(false) {
        world.walk_ins.retain(|x| x.buyer != buyer);
    }
}

/// § 2.4 (fixer, `gray::papers_daily` at midnight, the order paid): queue
/// `buyer` for the pickup in `fixer`'s open hours (once: a buyer already
/// queued keeps his place).
pub fn queue_walk_in(world: &mut World, buyer: EntityId, fixer: EntityId, forger: EntityId, tier: u8) {
    queue_commission(world, buyer, fixer, forger, tier, WalkKind::Papers);
}

/// Round 5 U5 (OPEN-217): queue a forger's commission of `kind` (once a
/// buyer: one already queued keeps his place).
pub fn queue_commission(
    world: &mut World,
    buyer: EntityId,
    fixer: EntityId,
    forger: EntityId,
    tier: u8,
    kind: WalkKind,
) {
    if world.walk_ins.iter().any(|w| w.buyer == buyer) {
        return;
    }
    let day = world.day();
    let unpaid = crate::systems::presence::on(world);
    world.walk_ins.push(WalkIn { buyer, fixer, forger, tier, day, unpaid, kind });
}

/// Is `buyer` queued for a walk-in?
pub fn walk_in_pending(world: &World, buyer: EntityId) -> bool {
    world.walk_ins.iter().any(|w| w.buyer == buyer)
}

/// Midnight (`gray::papers_daily`, after the queue): the walk-ins pending
/// from an earlier day are counted waited; each body queued is given
/// today's walk (a `Visit` with `VisitWhy::Papers` at the Fixer's first
/// shift hour) when the household pass is on and no other visit is due.
pub fn walk_ins_daily(world: &mut World) {
    let day = world.day();
    let waited = world.walk_ins.iter().filter(|w| w.day < day).count() as u32;
    ownership::jobs2_row(world).walk_ins_waited += waited;
    if !crate::systems::household::on(world) {
        return;
    }
    let due: Vec<WalkIn> = world.walk_ins.clone();
    for w in due {
        if world.comp::<Brain>(w.buyer).is_none_or(|b| b.lod == Lod::Statistical) || world.visits.contains_key(&w.buyer)
        {
            continue;
        }
        // Round 5 U5 (OPEN-217): the first open hour he is off his own
        // shift; none today, no walk (he comes on his rest day). The counter
        // keeps the day shift: a 09:00 visit waited out a 09-18 shift and
        // walked to a shut Fixer at 18:xx, every day (20k: 258 waited-days
        // against 29 served in 30 days).
        let Some(hour) = walk_in_hour_for(world, w.buyer, w.fixer, day) else { continue };
        let v = crate::systems::household::PendingVisit {
            kin: w.fixer,
            why: crate::systems::household::VisitWhy::Papers,
            hour,
        };
        world.visits.insert(w.buyer, v);
    }
}

/// Round 5 U5: the first hour of `fixer`'s open hours today (its staff's
/// shifts, `fixer_first_hour` to the latest shift's end) when `buyer` is
/// not on his own shift on a workday. `None`: no such hour.
fn walk_in_hour_for(world: &World, buyer: EntityId, fixer: EntityId, day: u64) -> Option<u8> {
    let first = fixer_first_hour(world, fixer)?;
    let end = world
        .staff_of(fixer)
        .iter()
        .filter_map(|&s| world.comp::<Job>(s))
        .flat_map(|j| j.shifts.iter().map(|&(_, b)| b))
        .max()
        .map_or(first, |m| u8::try_from(u64::from(m) / TICKS_PER_HOUR).unwrap_or(24));
    let Some(job) = world.comp::<Job>(buyer) else { return Some(first) };
    let start = day * crate::time::TICKS_PER_DAY;
    (first..end.max(first + 1)).find(|&h| {
        let tod = u16::from(h) * TICKS_PER_HOUR as u16;
        !(job.on_shift(tod)
            && crate::exec::routine::workday_of(world, buyer, job, job.shift_key_at(start + Tick::from(tod))))
    })
}

/// The first hour of `fixer`'s staff's earliest shift (its open hours' start).
pub fn fixer_first_hour(world: &World, fixer: EntityId) -> Option<u8> {
    world
        .staff_of(fixer)
        .iter()
        .filter_map(|&s| world.comp::<Job>(s))
        .flat_map(|j| j.shifts.iter().map(|&(a, _)| a))
        .min()
        .map(|m| u8::try_from(u64::from(m) / TICKS_PER_HOUR).unwrap_or(0))
}

/// § 2.4 (fixer, `household::credit`): the owner collected the week's cut
/// at his Fixer in person (`fixer_collections`): a `Client` encounter with
/// each member of staff present (the `Fixer` row), an event.
pub fn collected(world: &mut World, owner: EntityId, fixer: EntityId) {
    ownership::jobs2_row(world).fixer_collections += 1;
    let staff: Vec<EntityId> =
        world.staff_of(fixer).iter().copied().filter(|&s| s != owner && present(world, s, fixer)).collect();
    let d = world.district_of_building(fixer);
    for &s in &staff {
        encounter_post(world, "Fixer", owner, s, Some(fixer), d, ServeKind::Client, 0);
    }
    // The pre-view pass phase 4 (P3, OPEN-119): the cut held by the staff
    // present is handed to the owner now (the rest waits for them).
    if let Some(owed) = world.fixer_owed.remove(&fixer) {
        let mut left = Vec::new();
        for (holder, coins) in owed {
            if staff.contains(&holder) {
                ownership::pay(world, Some(holder), Some(owner), coins, Flow::Gray);
            } else if crate::systems::law::living(world, holder) {
                left.push((holder, coins));
            }
        }
        if !left.is_empty() {
            world.fixer_owed.insert(fixer, left);
        }
    }
    let text = format!("{} collected the week's cut at {} in person", world.name_of(owner), world.name_of(fixer));
    world.push_event(EventKind::Served, &[owner, fixer], text);
}

/// § 2.2 (civic clerk, `weapons::daily`'s licence sale): the clerk the
/// licence is sold by: among the civic clerks on the books at a standing
/// Hall, alive and free, the lowest `hash(PURPOSE_CLERK, buyer, day)`. The
/// sale runs at midnight, when no clerk is on shift: deviation, the desk is
/// named, not the hour's presence. `None` off or with no clerk.
pub fn licence_clerk(world: &World, buyer: EntityId) -> Option<(EntityId, EntityId)> {
    if !tower_on(world) {
        return None;
    }
    let role = world.config.trade_role("civic_clerk")?;
    let day = world.day();
    world
        .buildings_of_kind(BuildingKind::Hall)
        .iter()
        .copied()
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| !b.demolished))
        .flat_map(|h| world.staff_of(h).iter().map(move |&c| (c, h)))
        .filter(|&(c, _)| {
            world.comp::<Job>(c).is_some_and(|j| j.role == role)
                && crate::systems::law::living(world, c)
                && !world.has::<Sentence>(c)
        })
        .map(|(c, h)| (hash(world, PURPOSE_CLERK, key(buyer) ^ key(c), day), c, h))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, c, h)| (c, h))
}

/// § 2.2 (civic clerk): the licence sold by `clerk` at `hall` to `buyer`
/// (`licences_named`): a `Client` encounter (the fee) through the clerk's row.
pub fn licence_sold(world: &mut World, clerk: EntityId, hall: EntityId, buyer: EntityId, fee: i64) {
    ownership::jobs2_row(world).licences_named += 1;
    serve(world, clerk, buyer, hall, ServeKind::Client, fee);
}

/// § 2.2 (civic clerk, `gray::papers_daily`'s bribe): the clerk on the take
/// is named: a `Client` encounter (the bribe; the clerk's `Served` memory is
/// the on-the-take record), `bribes_named`. Tower only.
pub fn bribe_taken(world: &mut World, clerk: EntityId, buyer: EntityId, bribe: i64) {
    if !tower_on(world) {
        return;
    }
    ownership::jobs2_row(world).bribes_named += 1;
    let place = world.comp::<Job>(clerk).and_then(|j| j.employer);
    let d = place.map(|p| world.district_of_building(p)).unwrap_or_default();
    encounter(world, clerk, buyer, place, d, ServeKind::Client, bribe);
}

/// Lives at work phase 5: the weekday a Fixer's owner collects in person.
const PURPOSE_COLLECT: u64 = 0x5e52_000a;

/// Midnight (`living::run`, after `gray::daily`): the tower's day: the
/// walk-ins' walks and waits ([`walk_ins_daily`]) and the owners' weekly
/// collections: a Fixer's owner who holds no Job and is not inside it,
/// alive, free and adult, visits it on a hashed weekday (`hash(
/// PURPOSE_COLLECT, owner, week)`) at its first shift hour (a `Visit` with
/// `VisitWhy::Collect`; a Statistical owner credited at the hour by
/// `household::hourly`), unless another visit is due. Nothing off.
pub fn tower_daily(world: &mut World) {
    if !tower_on(world) {
        return;
    }
    walk_ins_daily(world);
    if !crate::systems::household::on(world) {
        return;
    }
    let day = world.day();
    let week = day / 7;
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    for f in fixers {
        if world.comp::<Building>(f).is_none_or(|b| b.demolished || b.derelict) {
            continue;
        }
        let Some(o) = world.owner_of(f) else { continue };
        if !world.has::<Brain>(o)
            || world.has::<Job>(o)
            || world.has::<Sentence>(o)
            || !crate::systems::law::living(world, o)
            || !crate::systems::demography::is_adult(world, o)
            || world.comp::<Brain>(o).is_some_and(|b| b.emigrating)
            || world.visits.contains_key(&o)
            || world.comp::<Position>(o).is_some_and(|p| p.building == Some(f))
        {
            continue;
        }
        let u = hash(world, PURPOSE_COLLECT, key(o) ^ key(f), week);
        if week * 7 + ((u * 7.0) as u64).min(6) != day {
            continue;
        }
        let Some(hour) = fixer_first_hour(world, f) else { continue };
        let v =
            crate::systems::household::PendingVisit { kin: f, why: crate::systems::household::VisitWhy::Collect, hour };
        world.visits.insert(o, v);
    }
}
