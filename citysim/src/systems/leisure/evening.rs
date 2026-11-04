use super::*;

// ---------------------------------------------------------------------------
// The Statistical evening (plan L17)
// ---------------------------------------------------------------------------

/// Jobs & lives phase 7b: the paid rungs open to `id` on its evening, one
/// venue per kind: from its day (its Home's door, or for a worker the
/// rectangle to its workplace's door; its Home's district first), a venue
/// within the Full ladder's `venue_walk_cap_ticks` of it, affordable from
/// the spendable purse (`stat_daily`'s, and `calibrate`'s eligibility for
/// `StatTable::p_venue`). Uncapped, the off-screen evening reached the
/// city's few vice venues from any district (Lounge visits 6.6x, StripClub
/// 2.6x the Full tier's on the calibration city), drained the purse, and
/// the food bought after it was synth.
fn evening_cands(world: &World, id: EntityId) -> Vec<Cand> {
    let Some((day_from, district)) = evening_day(world, id) else { return Vec::new() };
    let reach = Some(world.config.leisure.venue_walk_cap_ticks);
    paid_candidates(world, id, day_from, spendable(world, id), Some(district), reach)
}

/// The agent's day (`evening_cands`): its Home's door (else its tile), or
/// for a worker the rectangle to its workplace's door; and the Home's
/// district.
pub(super) fn evening_day(world: &World, id: EntityId) -> Option<(Origin, DistrictId)> {
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let from = home
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| world.outside_door(b))
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))?;
    let work = world
        .comp::<crate::components::Job>(id)
        .and_then(|j| j.employer)
        .and_then(|e| world.comp::<Building>(e))
        .map(|bd| world.outside_door(bd));
    Some((work.map_or(Origin::At(from), |w| Origin::Day(from, w)), world.district_of(from)))
}

/// Jobs & lives phase 7b: the venues of `id`'s evening for `p_venue`, one
/// per kind of the ladder: the nearest standing venue of the kind to its
/// day (its Home's district first) within the Full ladder's walk cap that
/// is open, lets the agent in (the door policy, the room, a Den's table) and
/// is affordable, the high rung only for those who reach it.
pub(super) fn evening_venues(world: &World, id: EntityId) -> Vec<(BuildingKind, EntityId)> {
    evening_venues_for(world, id, spendable(world, id))
}

/// [`evening_venues`] on `purse`, the agent's `spendable` read by the caller.
fn evening_venues_for(world: &World, id: EntityId, purse: i64) -> Vec<(BuildingKind, EntityId)> {
    if purse < 1 {
        return Vec::new();
    }
    let Some((day_from, district)) = evening_day(world, id) else { return Vec::new() };
    let cap = world.config.leisure.venue_walk_cap_ticks;
    let high = high_ok(world, id);
    let mut out = Vec::new();
    let max_d = reach_tiles(world, id, Some(cap));
    for kind in LADDER {
        if door_bars_kind(world, id, kind) {
            continue;
        }
        // The perf pass phase 2: the lowest `(other district, door distance,
        // id)` over the venues that pass, as the old sort of them all had it:
        // the nearest that passes in the Home's district, else the nearest
        // that passes outside it, each off the kind's grid.
        let pass = |b: EntityId| {
            standing(world, b)
                && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
                // Phase-8 review: the Full ladder's gates (a closed,
                // swept, dark, raided, unstaffed or unbuilt venue takes
                // nobody; the door policy; the room; a shut table).
                && open(world, b)
                && door_seen(world, id, b)
                && has_room(world, id, b)
                && (kind != BuildingKind::Den || table_open(world, b))
                && world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::exec::timed_walk(world, id, day_from.near(bd.door), bd.door) <= cap)
        };
        let Some((_, b)) = venue_by_distance(world, kind, day_from, max_d, |_, b| {
            world.district_of_building(b) == district && pass(b)
        })
        .or_else(|| {
            venue_by_distance(world, kind, day_from, max_d, |_, b| world.district_of_building(b) != district && pass(b))
        }) else {
            continue;
        };
        let price = price_of(world, b);
        if (!high && rung_of(world, b, price) == Rung::High && !lonely_client(world, id, b))
            || (kind != BuildingKind::Bar && 2 * price > purse)
        {
            continue;
        }
        out.push((kind, b));
    }
    out
}

/// The venue kinds of [`evening_venues`] (`calibrate`'s eligible agent-days
/// per kind).
pub fn evening_kinds(world: &World, id: EntityId) -> Vec<BuildingKind> {
    evening_venues(world, id).into_iter().map(|(k, _)| k).collect()
}

/// The `StatTable::p_venue` key of a venue kind.
pub fn venue_key(kind: BuildingKind) -> String {
    format!("{kind:?}")
}

const PURPOSE_STAT_VENUE: u64 = 0x4c45_0017;

/// Plan L17, 21:00: the Statistical evening. With the table's `p_venue`
/// (Jobs & lives phase 7b), every Statistical adult takes each paid rung of
/// [`evening_cands`] on the kind's learned rate `p` (the Full tier's paid
/// visits of the kind per eligible agent-day): `floor(p)` visits and one
/// more on the fraction (a pure hash of the agent, the day and the kind),
/// while the purse affords them; an adult below
/// `fun_satisfied` who took none has the free rung (`free_stat`). Without
/// it, every adult below `fun_satisfied`, ascending, takes the best rung
/// `choice` would, the free one when no paid rung outscores an hour's
/// HangOut. A visit pays as on screen, gains the rung's fun and adds a
/// visit; a table game's draw on `WordNs::Leisure` keyed `(day, id)`.
pub fn stat_daily(world: &mut World) {
    let day = world.day();
    let ids: Vec<EntityId> = world.tier(Lod::Statistical).to_vec();
    let g = world.config.leisure.gain.clone();
    if let Some(pv) = world.stat_table.as_ref().and_then(|t| t.p_venue.clone()) {
        stat_learned_evening(world, &ids, &pv);
        return;
    }
    for id in ids {
        if unwind_satisfied(world, id) {
            continue;
        }
        let cands = evening_cands(world, id);
        let pick = cands.iter().find(|c| c.act == ActionKind::EatOut).cloned().or_else(|| {
            cands.iter().max_by(|a, b| a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue))).cloned()
        });
        // The free rung's on-screen score (an hour's HangOut, no walk).
        let free_score = g.hangout_hour * world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        let Some(c) = pick.filter(|c| c.score > free_score) else {
            add_fun(world, id, g.free_stat);
            continue;
        };
        let Some(b) = c.venue else { continue };
        if queue_at_counter(world, id, b, c.act, None, 0) {
            continue;
        }
        let mut rng = world.rng.word(WordNs::Leisure, day, u64::from(id.index));
        stat_visit(world, id, b, c.act, None, &mut rng);
    }
}

/// The pre-view pass phase 3 (OPEN-145): an off-screen visit drawn into a
/// counter body's shift (`counter_due`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CounterVisit {
    /// The slot's tick (a multiple of `counter_settle_window` minutes).
    pub at: Tick,
    pub who: EntityId,
    pub venue: EntityId,
    pub act: ActionKind,
    /// The evening's purse the visit was rolled on (`stat_visit`'s `evening`).
    pub evening: Option<i64>,
    /// The day it was rolled and its ordinal that day (the visit's stream).
    pub day: u64,
    pub k: u32,
}

const PURPOSE_COUNTER: u64 = 0x4c45_0018;
/// Round 7 W6: the barred off-screen face back at the door.
const PURPOSE_BARRED_BACK: u64 = 0x4c45_0019;

/// The pre-view pass phase 3 (OPEN-145, P3): `b`'s counter bodies, the
/// staff of a `counter` shape that are bodies (not Statistical), living,
/// not held: ascending.
pub fn counter_bodies(world: &World, b: EntityId) -> Vec<EntityId> {
    world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            // Round 4 T4 (OPEN-217): a serving body of `[verbs] night_rows`
            // (a dancer, a club hand) draws the venue's off-screen visits
            // into its shift as a counter body does (a Club with only a body
            // Host took its whole evening in one 21:00 batch).
            (world
                .comp::<crate::components::Job>(s)
                .is_some_and(|j| crate::systems::shift::shape_of(world, j.role) == crate::config::ShiftShape::Counter)
                || crate::systems::service::night_row(world, s, false))
                && world.comp::<Brain>(s).is_some_and(|br| br.lod != Lod::Statistical)
                && !world.has::<crate::components::Sentence>(s)
                && crate::systems::law::living(world, s)
        })
        .collect()
}

/// The pre-view pass phase 3 (OPEN-145): the 21:00 evening's visit `k` of
/// `id` at `b`, when `b` has a counter body, is drawn (purpose-keyed: the
/// agent, the venue, the day, `k`) into a `counter_settle_window` slot of
/// the next 24 hours inside the bodies' shifts and queued (`counter_due`
/// pays and serves it then, so the day cook has the day's customers and a
/// barkeep serves them as they come, not in one 21:00 batch). `true` when
/// queued; with no counter body the visit is the evening's, as before (an
/// off-screen venue keeps its economy: the mirror).
fn queue_at_counter(
    world: &mut World,
    id: EntityId,
    b: EntityId,
    act: ActionKind,
    evening: Option<i64>,
    k: u32,
) -> bool {
    let Some(window) = world.config.shift.as_ref().and_then(|c| c.counter.as_ref()).map(|c| c.counter_settle_window)
    else {
        return false;
    };
    let bodies = counter_bodies(world, b);
    if bodies.is_empty() {
        return false;
    }
    // The union of the bodies' shift minutes over the next 24 hours.
    let now = world.tick;
    let day0 = now - now % TICKS_PER_DAY;
    let mut spans: Vec<(Tick, Tick)> = Vec::new();
    for &s in &bodies {
        let Some(j) = world.comp::<crate::components::Job>(s) else { continue };
        for &(a, e) in &j.shifts {
            for d in [day0, day0 + TICKS_PER_DAY] {
                let (lo, hi) = ((d + Tick::from(a)).max(now), (d + Tick::from(e)).min(now + TICKS_PER_DAY));
                if lo < hi {
                    spans.push((lo, hi));
                }
            }
        }
    }
    spans.sort_unstable();
    let mut union: Vec<(Tick, Tick)> = Vec::new();
    for (lo, hi) in spans {
        match union.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => union.push((lo, hi)),
        }
    }
    let total: Tick = union.iter().map(|&(lo, hi)| hi - lo).sum();
    if total == 0 {
        return false;
    }
    let day = world.day();
    let u = crate::systems::econ::hash_unit(
        world.seed(),
        PURPOSE_COUNTER,
        u64::from(id.index) << 24 ^ u64::from(b.index),
        day * 64 + u64::from(k),
    );
    let mut off = ((u * total as f32) as Tick).min(total - 1);
    let mut at = now;
    for &(lo, hi) in &union {
        if off < hi - lo {
            at = lo + off;
            break;
        }
        off -= hi - lo;
    }
    let w = Tick::from(window.max(1));
    let at = (at - at % w).max(now);
    world.counter_due.push(CounterVisit { at, who: id, venue: b, act, evening, day, k });
    crate::systems::ownership::jobs2_row(world).counter_queued += 1;
    true
}

/// The pre-view pass phase 3 (OPEN-145): every `counter_settle_window`
/// minutes, the queued visits due now are made (`stat_visit`: paid, the
/// fun, the visit, and the encounter at the counter, `service::arrive`,
/// served by whoever is behind it). A visitor no longer Statistical (a body
/// now, planning its own evenings) or dead drops its visit.
pub fn counter_due(world: &mut World) {
    if world.counter_due.is_empty() {
        return;
    }
    let w = world.config.shift.as_ref().and_then(|c| c.counter.as_ref()).map_or(1, |c| c.counter_settle_window.max(1));
    if !world.tick.is_multiple_of(Tick::from(w)) {
        return;
    }
    let now = world.tick;
    let (due, rest): (Vec<CounterVisit>, Vec<CounterVisit>) =
        std::mem::take(&mut world.counter_due).into_iter().partition(|v| v.at <= now);
    world.counter_due = rest;
    for v in due {
        if world.comp::<Brain>(v.who).is_none_or(|b| b.lod != Lod::Statistical)
            || !crate::systems::law::living(world, v.who)
            || !world.has::<Building>(v.venue)
            || !open(world, v.venue)
        {
            continue;
        }
        let mut rng = world.rng.word(WordNs::Leisure, v.day, u64::from(v.who.index) ^ (u64::from(v.k) + 1) << 40);
        stat_visit(world, v.who, v.venue, v.act, v.evening, &mut rng);
    }
}

/// An adult's evening: its purse and its venues (`stat_learned_evening`).
type EveningPlan = (EntityId, i64, Vec<(BuildingKind, EntityId)>);

/// Jobs & lives phase 7b: the learned evening (`StatTable::p_venue`, the
/// Full tier's paid visits of each kind per adult-day). The tier's adults
/// and each one's evening venues (`evening_venues`, read before anyone
/// pays) are counted; the kind's visits due are its rate times the adults,
/// shared over the adults eligible for it: each takes `r = rate x adults /
/// eligible` visits (`floor(r)` and one more on the fraction, a pure hash
/// of the agent, the day and the kind), in a keyed order, while the
/// evening's purse affords the rung. So the tier pays the kinds as often as
/// the Full tier does, at the venues its people live and work by. An adult
/// below `fun_satisfied` who took none has the free rung.
fn stat_learned_evening(world: &mut World, ids: &[EntityId], pv: &std::collections::BTreeMap<String, f32>) {
    let day = world.day();
    let free = world.config.leisure.gain.free_stat;
    let adults: Vec<EntityId> =
        ids.iter().copied().filter(|&id| crate::systems::demography::is_adult(world, id)).collect();
    // The view milestone V2 (OPEN-265): the plan reads the world and writes
    // nothing, so it is read on the think slice's pool (`[perf] threads`),
    // in the adults' order: the 100k evening's ~750,000 venue queries were
    // one 21:00 tick of 4 s on one thread.
    let plan_of = |id: EntityId| {
        let purse = spendable(world, id);
        (id, purse, evening_venues_for(world, id, purse))
    };
    let threads = world.config.perf.threads.max(1);
    let plan: Vec<EveningPlan> = if threads == 1 {
        adults.iter().map(|&id| plan_of(id)).collect()
    } else {
        use rayon::prelude::*;
        #[cfg(test)]
        let scans = crate::spatial::scans();
        crate::systems::think::pool(threads).install(|| {
            adults
                .par_iter()
                .map(|&id| {
                    #[cfg(test)]
                    crate::spatial::SCANS.with(|s| s.set(scans));
                    plan_of(id)
                })
                .collect()
        })
    };
    let adults = plan.len() as f32;
    let mut eligible = [0u32; LADDER.len()];
    for (_, _, venues) in &plan {
        for &(k, _) in venues {
            if let Some(i) = LADDER.iter().position(|&x| x == k) {
                eligible[i] += 1;
            }
        }
    }
    for (id, evening, mut venues) in plan {
        // In a keyed order (the ladder's order priced the dear rungs out
        // of a purse the cheap ones had spent).
        venues.sort_by_key(|&(k, _)| {
            let k = LADDER.iter().position(|&x| x == k).unwrap_or(0) as u64;
            splitmix64(world.seed() ^ PURPOSE_STAT_VENUE ^ (u64::from(id.index) << 20) ^ (day << 4) ^ k)
        });
        let mut rng = world.rng.word(WordNs::Leisure, day, u64::from(id.index));
        let mut went = false;
        for (kind, b) in venues {
            let i = LADDER.iter().position(|&k| k == kind).unwrap_or(0);
            // The pressure pass R6a (OPEN-217): the learned table has no
            // Dollhouse row (learned before the chip had clients: no
            // off-screen visit ever), so its rate is `[trade_verbs]`'s.
            let rate = pv
                .get(&venue_key(kind))
                .copied()
                .or_else(|| {
                    (kind == BuildingKind::Dollhouse)
                        .then(|| world.config.trade_verbs.as_ref().map(|c| c.dollhouse_p_venue))
                        .flatten()
                })
                .unwrap_or(0.0)
                .max(0.0);
            let mut r = rate * adults / eligible[i].max(1) as f32;
            // R6a: the few eligible for the Dollhouse (the lonely with the
            // purse, the rich) take at most a visit a night (three at 21:00).
            if kind == BuildingKind::Dollhouse && world.config.trade_verbs.is_some() {
                r = r.min(1.0);
            }
            let u = crate::systems::econ::hash_unit(
                world.seed(),
                PURPOSE_STAT_VENUE,
                u64::from(id.index),
                day * 16 + i as u64,
            );
            let n = r.floor() as u32 + u32::from(u < r.fract());
            for k in 0..n {
                // The pre-view pass phase 3 (OPEN-145): a venue with a
                // counter body serves the visit in the body's shift.
                if queue_at_counter(world, id, b, act_of(kind), Some(evening), (i as u32) << 8 | k) {
                    went = true;
                    continue;
                }
                if !stat_visit(world, id, b, act_of(kind), Some(evening), &mut rng) {
                    break;
                }
                went = true;
            }
        }
        if !went && !unwind_satisfied(world, id) {
            add_fun(world, id, free);
        }
    }
}

/// One off-screen paid visit of `id` at `b` (`stat_daily`): false when
/// nothing was paid (an emptied shelf, an empty purse). The rung must be
/// affordable from `evening` (the learned path: the purse the evening
/// opened with, as `evening_venues` read it; each rolled visit is one the
/// Full tier makes on its own hour's purse, not one the evening's earlier
/// visits priced out), else from the purse now; the coins come out of the
/// purse now.
/// Round 7 W6 (OPEN-217): does the off-screen face barred from `b` try its
/// door tonight anyway? Drunk, yes; else on `[customers] door_stat_barred_p`
/// (a hash of him, the venue and the day). A face turned away there inside
/// `[exec] refused_ticks` (`Brain.refused`) has had his answer.
fn barred_returns(world: &World, id: EntityId, b: EntityId) -> bool {
    let Some(p) = crate::systems::customers::cfg(world).map(|c| c.door_stat_barred_p) else { return false };
    if p <= 0.0 {
        return false;
    }
    let turned = world
        .comp::<Brain>(id)
        .and_then(|br| br.refused)
        .is_some_and(|(r, t)| r == b && world.tick.saturating_sub(t) < world.config.exec.refused_ticks.max(1));
    if turned {
        return false;
    }
    if crate::systems::misery::influence(world, id) == Some("drunk") {
        return true;
    }
    let key = u64::from(b.index) ^ world.day().rotate_left(23);
    crate::systems::econ::hash_unit(world.seed(), PURPOSE_BARRED_BACK, u64::from(id.index), key) < p
}

fn stat_visit(
    world: &mut World,
    id: EntityId,
    b: EntityId,
    act: ActionKind,
    evening: Option<i64>,
    rng: &mut impl Rng,
) -> bool {
    let now = spendable(world, id);
    let purse = evening.unwrap_or(now);
    let price = price_of(world, b);
    // Round 6 V6 (OPEN-217): at a kept door the bouncer meets the visitor
    // before his purse does: one come with the evening's purse spent is
    // turned away "broke" (`customers::cause`), not a silent stay-home.
    let counter = crate::systems::shift::counter_on(world);
    let door_first = counter
        && crate::systems::customers::cfg(world).is_some_and(|c| c.door_broke)
        && kind_of(world, b).is_some_and(|k| crate::systems::customers::kept_kind(world, k));
    // A visitor this door turned away tonight does not queue back into it
    // (the evening's later visits rolled before the refusal: the 100k
    // shadow's broke came back and were barred for the return).
    if door_first && !door_seen(world, id, b) {
        // Round 7 W6 (OPEN-217): a face the house barred comes back to its
        // door anyway on `[customers] door_stat_barred_p` (drunk, always)
        // and is turned away "barred" (V15: the 100k door's only reason
        // was "broke"); else he stays away (round 4 T7).
        if crate::systems::vice::barred(world, id, b) && barred_returns(world, id, b) {
            crate::systems::service::refused(world, id, b);
        }
        return false;
    }
    if door_first && !door_ok(world, id, b) {
        crate::systems::service::refused(world, id, b);
        return false;
    }
    if now < 1 || (act != ActionKind::Drink && 2 * price > purse) {
        return false;
    }
    // The pre-view pass phase 3 (OPEN-121, the mirror): the off-screen
    // visitor meets the same door a body does (`door_seen` picked it).
    if counter && !door_first && !door_ok(world, id, b) {
        crate::systems::service::refused(world, id, b);
        return false;
    }
    // Phase 5: the gate's probe (not a CSV column): the evening's gross
    // spend, entries, meals, drinks and stakes.
    let mut gross = 0i64;
    match act {
        ActionKind::Drink => {
            let owner = world.owner_of(b);
            let paid = ownership::pay(world, Some(id), owner, 2, Flow::Drink);
            ownership::credit(world, b, paid);
            note_paid(world, id, b, paid);
            gross += paid;
            add_fun(world, id, gain_at(world, b));
            add_belonging(world, id, 0.15);
            world.remember(id, MemoryKind::Socialised, None, 0.2, 0.3, false);
            // Round 6 V6 (OPEN-217): the drink's mark the door reads (the
            // off-screen memory keeps no `Socialised`: 9,264 off-screen
            // arrivals at 20k's kept doors, none drunk).
            if paid > 0 && crate::systems::customers::cfg(world).is_some_and(|c| c.door_stat_drink) {
                let now = world.tick;
                if !world.has::<crate::components::Misery>(id) {
                    world.insert(id, crate::components::Misery::default());
                }
                if let Some(m) = world.comp_mut::<crate::components::Misery>(id) {
                    m.drank_at = now;
                }
            }
            // Jobs & lives phase 5 (§ 5.3): a black unit from the Bar's dealer (nothing off).
            crate::systems::braindance::bar_visit(world, id, b, false);
        }
        ActionKind::Gamble => {
            // The stake from the purse now, as a Full round's.
            let stake = stake_of(world, now);
            let u: f32 = rng.random();
            play_table(world, id, b, stake, u);
            gross += stake;
            add_fun(world, id, gain_at(world, b));
            crate::systems::jobs::note_visit(world, b);
        }
        ActionKind::EatOut => {
            // Review fix: the shelf first (emptied earlier in this pass,
            // there is no meal: no pay, no fun, no visit), then the pay.
            if world.comp::<Building>(b).is_none_or(|bd| bd.stock_food == 0) {
                return false;
            }
            let paid = pay_venue(world, id, b, price, Flow::Leisure);
            if paid <= 0 {
                return false;
            }
            gross += paid;
            world.take_stock(b, crate::components::Good::Food, 1);
            let cfg = world.config.needs.clone();
            if let Some(n) = world.comp_mut::<Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            // Jobs & lives phase 7b: the meal as on screen (`on_complete`'s
            // EatOut): the venue's scraps, a real meal, the day's mark.
            crate::systems::bins::venue_meal(world, b);
            crate::systems::health::ate(world, id, crate::components::MealKind::Real);
            world.mark_day(id, crate::components::trace_flags::ATE);
            add_fun(world, id, gain_at(world, b));
            crate::systems::jobs::note_visit(world, b);
        }
        _ => {
            gross += pay_venue(world, id, b, price, Flow::Leisure);
            add_fun(world, id, gain_at(world, b));
            add_belonging(world, id, belonging_at(world, b));
            // A FightPit evening off screen bets as a table (the
            // Leisure stream), when brave enough and in funds. Not on the
            // learned path (phase 7b: the Full tier bet at a pit once in
            // three calibration cities against 1,320 pit visits).
            // The pressure pass R6a (OPEN-217): in the bet window the brave
            // back a fighter in tonight's bout (`bout_bet`), both paths; the
            // table game above only stood in for it (V10: every bout "bets
            // paid 0": the bettors were the Full tier's, who plan the
            // pit before the window opens).
            let bet = if kind_of(world, b) == Some(BuildingKind::FightPit) { bout_bet(world, id, b) } else { 0 };
            if bet > 0 {
                gross += bet;
            } else if evening.is_none()
                && kind_of(world, b) == Some(BuildingKind::FightPit)
                && !world.levers.ban_gambling
            {
                let brave = world
                    .comp::<Personality>(id)
                    .is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
                let left = spendable(world, id);
                if brave && left >= 1 {
                    let stake = stake_of(world, left);
                    let u: f32 = rng.random();
                    play_table(world, id, b, stake, u);
                    gross += stake;
                }
            }
            world.remember(id, MemoryKind::Enjoyed, None, 0.2, 0.3, false);
            crate::systems::jobs::note_visit(world, b);
            phase4_visit(world, id, b);
        }
    }
    world.stats.current.living.stat_spend += gross;
    // Lives at work phase 1 (§ 2.1): the off-screen visit queues at the
    // same counter a body's does (the mirror by construction).
    if gross > 0 {
        crate::systems::service::arrive(world, id, b, crate::components::ServeKind::Visit, gross);
    }
    gross > 0
}
