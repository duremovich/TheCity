use super::*;

// ---------------------------------------------------------------------------
// The street (on the phase-2 Pitch)
// ---------------------------------------------------------------------------

/// The district's pimp, free and alive (his gang's protection).
pub(super) fn pimp_of(world: &World, d: DistrictId) -> Option<EntityId> {
    let p = *world.jobs2.vice.pimps.get(&(d.index() as u16))?;
    (crate::systems::law::living(world, p) && !world.has::<Sentence>(p) && world.gang_of(p).is_some()).then_some(p)
}

/// `streetwork::close_the_day`: a street pitch under a pimp pays his gang
/// `pimp_cut` of its day instead of the holding gang's `cut_frac`.
///
/// Lives at work phase 4b (§ 2.3): with `[service] street` on, the cut is
/// the pimp's (`StreetCut::pimp`): `close_the_day` pays him and he passes
/// `pimp_pass_up` of it to the gang the same tick (`pass_up`).
pub fn street_cut(world: &World, p: &Pitch) -> Option<StreetCut> {
    if !on(world) || !is_street_pitch(world, p) {
        return None;
    }
    let pimp = pimp_of(world, p.district)?;
    let gang = world.gang_of(pimp)?;
    let to_pimp = crate::systems::service::street_on(world);
    Some(StreetCut { gang, pimp: to_pimp.then_some(pimp), frac: world.config.vice.pimp_cut })
}

/// `street_cut`'s answer: the gang, the pimp when the cut is his (the
/// street switch on), the share of the pitch's day.
#[derive(Clone, Copy, Debug)]
pub struct StreetCut {
    pub gang: EntityId,
    pub pimp: Option<EntityId>,
    pub frac: f32,
}

/// Lives at work phase 4b (§ 2.3): the pimp, paid his cut of a street
/// pitch's day, passes `pimp_pass_up` of it to his gang (`Flow::StreetCut`,
/// the same tick). Returns the coins passed up.
pub fn pass_up(world: &mut World, pimp: EntityId, gang: EntityId, paid: i64) -> i64 {
    let up = (paid as f32 * world.config.service.pimp_pass_up).round() as i64;
    if up <= 0 {
        return 0;
    }
    let passed = ownership::pay(world, Some(pimp), Some(gang), up, Flow::StreetCut);
    ownership::jobs2_row(world).pimp_pass_up += passed;
    passed
}

/// Lives at work phase 4b (§ 2.3): the pimp of district `d` is present at
/// the pitch on `spot` now: his body (Full or Coarse) out within
/// `pimp_tiles` of it, or, a Statistical pimp (no Job, so no
/// `shift_attended`: the deviation), credited while he follows his gang's
/// order (`gang::following_order`). A presence marks `pimp_seen`.
pub fn pimp_present(world: &mut World, d: DistrictId, spot: TilePos) -> bool {
    let Some(pimp) = pimp_of(world, d) else { return false };
    let here = match world.comp::<Brain>(pimp).map(|b| b.lod) {
        Some(Lod::Statistical) => crate::systems::gang::following_order(world, pimp).is_some(),
        Some(_) => world
            .comp::<Position>(pimp)
            .is_some_and(|p| p.building.is_none() && p.tile.manhattan(spot) <= world.config.service.pimp_tiles),
        None => false,
    };
    if here {
        let today = world.day();
        world.jobs2.vice.pimp_seen.insert(d.index() as u16, today);
    }
    here
}

/// The district a named pimp works (his `pimps` entry), if `id` is one.
pub fn pimp_district(world: &World, id: EntityId) -> Option<DistrictId> {
    let d = world.jobs2.vice.pimps.iter().find(|&(_, &p)| p == id).map(|(&d, _)| d)?;
    world.districts.get(usize::from(d)).map(|x| x.id)
}

/// Lives at work phase 4b (§ 2.3): the street worker whose pitch is the
/// pimp's turn this hour: his district's street pitches in their hours, by
/// owner, the hour's index of them (the round, in turn). `None` off, for a
/// non-pimp, out of the pitch hours.
pub fn round_pitch(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) || !crate::systems::service::street_on(world) {
        return None;
    }
    let d = pimp_district(world, id)?;
    pimp_of(world, d).filter(|&p| p == id)?;
    let due: Vec<EntityId> = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && is_street_pitch(world, p) && crate::systems::streetwork::due(world, p))
        // Round 5 U6: no pimp works a child (the opener is an adult; the
        // rule named here, `ages::is_protected`).
        .filter(|p| !crate::systems::ages::is_protected(world, p.owner))
        .map(|p| p.owner)
        .collect();
    if due.is_empty() {
        return None;
    }
    let hour = (world.tick / TICKS_PER_HOUR) as usize;
    // The pressure pass phase 2 (OPEN-120): a pimp's body walks to the
    // nearest pitch due, not the
    // hour's turn across the district (V9: 334-360 min legs, no collection).
    // R6a (OPEN-217): within `[trade_verbs] pimp_round_tiles` first (a
    // far pitch of a wide district was a six-hour walk), then a pitch not
    // collected at within `pimp_revisit_ticks`, one with a take owed before
    // one without, the nearest of those (the nearest alone kept him on one pitch all night: 12
    // collections from one worker in four nights, none from the other
    // worker of his district; the cut after the first was nothing).
    if crate::systems::customers::cfg(world).is_some()
        && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
    {
        if let Some(from) = world.comp::<Position>(id).map(|p| p.tile) {
            let now = world.tick;
            let tv = world.config.trade_verbs.as_ref().map(|c| (c.pimp_revisit_ticks, c.pimp_round_tiles));
            let rank = |o: &EntityId| {
                let Some(p) = world.jobs2.pitches.get(o) else { return (true, 3u8, u32::MAX) };
                let dist = p.spot.manhattan(from);
                match tv {
                    None => (false, 0, dist),
                    Some((r, reach)) => {
                        let seen = world.jobs2.vice.pimp_visits.get(&key(*o)).is_some_and(|&t| now < t + r);
                        (dist > reach, 2 * u8::from(seen) + u8::from(p.take_today <= p.cut_paid), dist)
                    }
                }
            };
            // Pressure pass round 3, S1 (OPEN-221): the pick past
            // `[distance.choice] gang_work_ticks` of him is no round now
            // (V11: the pimp's legs of 398 tiles, cut, no Collect).
            let cap = pimp_trip_cap(world);
            return due.iter().copied().min_by_key(|o| (rank(o), *o)).filter(|o| {
                cap.is_none_or(|c| {
                    world
                        .jobs2
                        .pitches
                        .get(o)
                        .is_some_and(|p| crate::systems::distance::trip(world, id, from, p.spot) <= c)
                })
            });
        }
    }
    Some(due[hour % due.len()])
}

/// Round 5 U6 (OPEN-217): the round's trip bound, a commute's, not an
/// errand's: `[trade_verbs] pimp_trip_ticks` (S1's `[distance.choice]
/// gang_work_ticks` without it); `None` without `[distance.choice]`.
pub(super) fn pimp_trip_cap(world: &World) -> Option<crate::time::Tick> {
    let c = crate::systems::distance::choice(world)?;
    Some(world.config.trade_verbs.as_ref().map_or(c.gang_work_ticks, |t| t.pimp_trip_ticks))
}

/// `gang::gang_work_target` for a pimp's body: the worker of his round's
/// pitch this hour (the GangWork goal's target; the plan is `pimp_plan`).
pub fn pimp_target(world: &World, id: EntityId) -> Option<EntityId> {
    if world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical) {
        return None;
    }
    round_pitch(world, id)
}

/// GangWork's scripted plan for a pimp's body in pitch hours: `GoTo(Spot)`
/// (the round's pitch) then `Collect` (an hour at it, `collect_ticks`).
pub fn pimp_plan(world: &mut World, id: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::{ActionInstance, GoalKind};
    use crate::goap::{ActionKind, LocationKey, Plan};
    let worker = pimp_target(world, id)?;
    let spot = world.jobs2.pitches.get(&worker)?.spot;
    world.unwind.insert(
        id,
        crate::living::UnwindPick {
            rung: crate::living::Rung::Free,
            act: ActionKind::Collect,
            venue: None,
            spot: Some(spot),
            score: 0.0,
            stay_in: false,
        },
    );
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot), None), step(ActionKind::Collect, Some(worker))];
    Some(Plan { goal: GoalKind::GangWork, target: Some(worker), steps, started_tick: world.tick })
}

/// A pimp's `Collect` at `worker`'s pitch (`leisure::can_start`): `Some`
/// when this is the pimp's collection (his district's street pitch as the
/// target), then whether he stands out within `pimp_tiles` of it.
pub fn collect_can_start(world: &World, id: EntityId, target: Option<EntityId>) -> Option<bool> {
    let worker = target?;
    let p = world.jobs2.pitches.get(&worker)?;
    if !is_street_pitch(world, p) || pimp_district(world, id) != Some(p.district) {
        return None;
    }
    let r = world.config.service.pimp_tiles;
    Some(world.comp::<Position>(id).is_some_and(|x| x.building.is_none() && x.tile.manhattan(p.spot) <= r))
}

/// The pimp's `Collect` runs the hour at the pitch (presence is what
/// protects it); `None` for any other Collect (and off).
pub fn collect_ticks(world: &World, id: EntityId) -> Option<crate::time::Tick> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    pimp_district(world, id).map(|_| TICKS_PER_HOUR)
}

/// A pimp's `Collect` ends (`leisure::on_complete`): the collection, a
/// `Staff` encounter with the worker (`"pimp"`'s row), `pimp_collections`.
/// `false` when it was not the pimp's (the leader's Collect runs on).
pub fn pimp_collect(world: &mut World, id: EntityId, target: Option<EntityId>) -> bool {
    let Some(worker) = target else { return false };
    let Some(p) = world.jobs2.pitches.get(&worker).cloned() else { return false };
    if !is_street_pitch(world, &p) || pimp_district(world, id) != Some(p.district) {
        return false;
    }
    // R6a: the visit marks the pitch for the round, met or missed.
    if world.config.trade_verbs.is_some() {
        let now = world.tick;
        world.jobs2.vice.pimp_visits.insert(key(worker), now);
    }
    // The pre-view pass phase 4 (P3, OPEN-120): the collection needs both
    // bodies: the worker at his pitch (`streetwork::working`: a body within
    // `pitch_reach` of the spot, a Statistical one on the hour's draw), else
    // no "collected" line in either diary (the next hour's pitch is tried).
    // R6a (OPEN-217): an off-screen worker in her pitch hours (now or at the
    // hour's start) is met by the pimp's body that stood the hour at her
    // pitch (the hour's draw stands in for the passers-by; on it, a round of
    // three hours' walk met nobody).
    let met = if world.config.trade_verbs.is_some()
        && world.comp::<Brain>(worker).is_some_and(|b| b.lod == Lod::Statistical)
    {
        let start = (u64::from(world.tick_of_day()) + TICKS_PER_DAY - TICKS_PER_HOUR) % TICKS_PER_DAY;
        crate::systems::streetwork::due(world, &p) || crate::systems::streetwork::due_at(world, &p, start as u16)
    } else {
        crate::systems::streetwork::working(world, &p)
    };
    if crate::systems::presence::on(world) && !met {
        crate::systems::presence::missed(world);
        return true;
    }
    collection(world, id, worker, p.district);
    true
}

pub(super) fn collection(world: &mut World, pimp: EntityId, worker: EntityId, d: DistrictId) {
    ownership::jobs2_row(world).pimp_collections += 1;
    world.jobs2.vice.collected.insert(key(pimp));
    let kind = crate::components::ServeKind::Staff;
    crate::systems::service::encounter_post(world, "pimp", pimp, worker, None, d, kind, 0);
    let booked = book_client(world, pimp, worker, d);
    let cut = cut_at_meeting(world, pimp, worker);
    tell_collection(world, pimp, worker, booked, cut);
}

/// `"Name (N y)"`.
fn aged(world: &World, id: EntityId) -> String {
    let age = world.comp::<crate::components::Identity>(id).map_or(0, |i| i.age_years());
    format!("{} ({age} y)", world.name_of(id))
}

/// Round 5 U6 (OPEN-217): the pimp at a dry pitch (nothing taken since his
/// last collection; his body, or an off-screen pimp credited present) sends
/// her a client: the first of
/// `[trade_verbs] pimp_book_draws` off-screen residents of the district (a
/// hash of the pair and the hour) who is an adult and not protected
/// (`ages::is_protected`: no child is ever a client), free, fed (hunger 0.5
/// or more), under `[vice] street_intimacy` and holding twice the price
/// (`street_payer`'s rule). He pays the worker `street_price` (the visit's
/// `Leisure`, the take), the pair's `Client` encounter. Returns the client
/// and the coins. V13: the street's off-screen payers were three draws an
/// hour, and a pimp's round collected 2c or nothing (82 street visits in 30
/// days at 20k); a pitch dry `[jobs2] pitch_fail_days` closed, and a pimp's
/// district emptied (`pimp_18110`'s round stopped on d23).
fn book_client(world: &mut World, pimp: EntityId, worker: EntityId, d: DistrictId) -> Option<(EntityId, i64)> {
    let draws = world.config.trade_verbs.as_ref().map_or(0, |c| c.pimp_book_draws);
    if draws == 0 || crate::systems::ages::is_protected(world, worker) || !crate::systems::service::street_on(world) {
        return None;
    }
    let p = world.jobs2.pitches.get(&worker)?;
    if p.take_today > p.cut_paid {
        return None;
    }
    let res = world.districts.get(d.index())?.residents.clone();
    if res.is_empty() {
        return None;
    }
    let cfg = world.config.vice.clone();
    let hour = world.tick / TICKS_PER_HOUR;
    let client = (0..u64::from(draws)).find_map(|k| {
        let u = hash(world, PURPOSE_BOOK, key(pimp) ^ key(worker), hour * 16 + k);
        let id = res[((u * res.len() as f32) as usize).min(res.len() - 1)];
        (id != worker
            && id != pimp
            && !crate::systems::presence::is_body(world, id)
            && crate::systems::demography::is_adult(world, id)
            && !crate::systems::ages::is_protected(world, id)
            && !world.has::<Sentence>(id)
            && crate::systems::law::living(world, id)
            && world.comp::<Needs>(id).is_some_and(|n| n.intimacy < cfg.street_intimacy && n.hunger >= 0.5)
            && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 2 * cfg.street_price))
        .then_some(id)
    })?;
    let paid = ownership::pay(world, Some(client), Some(worker), cfg.street_price, Flow::Leisure);
    if paid <= 0 {
        return None;
    }
    add_intimacy(world, client, cfg.intimacy_gain);
    if let Some(x) = world.jobs2.pitches.get_mut(&worker) {
        x.take_today += paid;
    }
    let row = ownership::jobs2_row(world);
    row.street_visits += 1;
    row.street_take += paid;
    row.pimp_booked += 1;
    let kind = crate::components::ServeKind::Client;
    crate::systems::service::encounter(world, worker, client, None, d, kind, 0);
    Some((client, paid))
}

/// Round 5 U6 (OPEN-217): the collection told, in both diaries, when the
/// cut moved at the meeting (a body on either side): the client he sent,
/// the coins taken since his last round, his cut, the gang's share of it.
/// A dry pitch with no client found tells nothing (the round's next pitch).
fn tell_collection(
    world: &mut World,
    pimp: EntityId,
    worker: EntityId,
    booked: Option<(EntityId, i64)>,
    cut: Option<(i64, i64, i64)>,
) {
    let Some((due, paid, up)) = cut else { return };
    let price = world.config.vice.street_price.max(1);
    let clients = (due + price - 1) / price;
    let gang = world.gang_of(pimp).and_then(|g| world.comp::<Gang>(g)).map(|g| g.name.clone());
    let up = match gang {
        Some(g) if up > 0 => format!(", {up}c of it up to {g}"),
        _ => String::new(),
    };
    let (pn, wn) = (world.name_of(pimp).to_string(), aged(world, worker));
    let text = match booked {
        Some((c, coins)) => format!(
            "{pn} sent {} to the pitch of {wn} ({coins}c) and took the cut: {paid}c of {due}c since the \
             last round{up}",
            aged(world, c)
        ),
        None => format!(
            "{pn} came round the pitch of {wn} and took the cut: {paid}c of {due}c since the last round ({clients} \
             client{}){up}",
            if clients == 1 { "" } else { "s" }
        ),
    };
    let mut actors = vec![pimp, worker];
    if let Some((c, _)) = booked {
        actors.push(c);
    }
    world.push_event(EventKind::Served, &actors, text);
}

/// The pre-view pass phase 4 (P3, OPEN-119): with `[presence]` on and a
/// body on either side, the pimp's cut is taken when he meets the worker:
/// `pimp_cut` of the take since the last collection (`Pitch.cut_paid`),
/// worker -> pimp (`Flow::StreetCut`), `pimp_pass_up` of it to the gang.
/// A Statistical-only pair keeps the day's close (`street_cut_at_close`).
/// Round 5 U6: returns `(the take since the last collection, the cut paid,
/// the coins passed up to the gang)` when a cut moved.
fn cut_at_meeting(world: &mut World, pimp: EntityId, worker: EntityId) -> Option<(i64, i64, i64)> {
    if !crate::systems::presence::on(world) || !body_pair(world, pimp, worker) {
        return None;
    }
    let p = world.jobs2.pitches.get(&worker).cloned()?;
    let c = street_cut(world, &p)?;
    let due = p.take_today - p.cut_paid;
    if due <= 0 {
        return None;
    }
    let cut = (due as f32 * c.frac).round() as i64;
    let to = c.pimp.unwrap_or(c.gang);
    let paid = ownership::pay(world, Some(worker), Some(to), cut, Flow::StreetCut);
    if let Some(x) = world.jobs2.pitches.get_mut(&worker) {
        x.cut_paid = p.take_today;
        x.cut_to = Some(c.gang);
    }
    let row = ownership::jobs2_row(world);
    row.street_cut += paid;
    row.pimp_cut += paid;
    row.cut_at_meet += 1;
    let up = if to != c.gang { pass_up(world, to, c.gang, paid) } else { 0 };
    Some((due, paid, up))
}

/// Either side a body (Full or Coarse).
fn body_pair(world: &World, a: EntityId, b: EntityId) -> bool {
    crate::systems::presence::is_body(world, a) || crate::systems::presence::is_body(world, b)
}

/// `streetwork::close_the_day`: does the pimp's cut of this pitch still
/// settle at the day's close? Off, or a Statistical-only pair (the mirror);
/// with a body on either side it was taken at the collections.
pub fn street_cut_at_close(world: &World, p: &Pitch, c: &StreetCut) -> bool {
    !crate::systems::presence::on(world) || !body_pair(world, c.pimp.unwrap_or(c.gang), p.owner)
}

/// A Statistical pimp credited present collects at the pitch whose turn
/// the hour is (`round_pitch`), as a body's `Collect` would.
pub(super) fn stat_collection(world: &mut World, p: &Pitch) {
    let Some(pimp) = pimp_of(world, p.district) else { return };
    if world.comp::<Brain>(pimp).is_none_or(|b| b.lod != Lod::Statistical) {
        return;
    }
    if round_pitch(world, pimp) == Some(p.owner) {
        collection(world, pimp, p.owner, p.district);
    }
}
