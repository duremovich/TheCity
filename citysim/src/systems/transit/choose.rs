use super::*;

// ---------------------------------------------------------------------------
// The route and the choice (T7, T8)
// ---------------------------------------------------------------------------

/// The `stop_candidates` open stops nearest `tile` at `t`, by Manhattan,
/// ties the lower index.
pub(super) fn candidates(world: &World, tile: TilePos, t: Tick) -> Vec<u16> {
    let k = world.config.transit.stop_candidates.max(1);
    // Phase 2: the tile's precomputed shortlist, in the scan's order; the
    // scan below only when closures leave the shortlist short.
    let net = &world.transit.net;
    let near = net.near(tile);
    let open: Vec<u16> = near.iter().copied().filter(|&i| stop_open(world, i, t)).take(k).collect();
    if open.len() == k || near.len() == net.stops.len() {
        return open;
    }
    let mut v: Vec<(u32, u16)> = world
        .transit
        .net
        .stops
        .iter()
        .enumerate()
        .filter_map(|(i, &s)| u16::try_from(i).ok().map(|i| (s.manhattan(tile), i)))
        .collect();
    v.sort_unstable();
    // The open test (a district lookup, the riots) only for the nearest.
    v.into_iter().filter(|&(_, i)| stop_open(world, i, t)).take(k).map(|(_, i)| i).collect()
}

/// T7: the fastest ride from `from` to `to` for `who` leaving at `t`, door to
/// door: walk to a candidate board stop, the exact next departure, the
/// table's ride, the walk from the alight stop. `None` without one.
pub fn route(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Option<Route> {
    let board = candidates(world, from, t);
    let alight = candidates(world, to, t);
    let net = &world.transit.net;
    let mut best: Option<Route> = None;
    for &s in &board {
        let w1 = crate::exec::timed_walk(world, who, from, net.stops[usize::from(s)]);
        if !access_ok(world, w1) {
            continue;
        }
        for &e in &alight {
            if s == e || passes(world, from, s, e) {
                continue;
            }
            let hop = net.hop(s, e);
            if hop.ticks == UNREACHABLE {
                continue;
            }
            // The island pass phase 6: with trains, every leg scheduled
            // (a transfer waits for its train).
            let (depart, arrive) = if trains_on(world) {
                let Some(x) = legs_of(world, s, e).and_then(|legs| schedule(world, &legs, t + w1)) else { continue };
                x
            } else {
                let Some(depart) = departure_at(world, hop.line, hop.forward, hop.pos, t + w1) else { continue };
                (depart, depart + Tick::from(hop.ticks))
            };
            if !in_service(world, depart) {
                continue;
            }
            let w2 = crate::exec::timed_walk(world, who, net.stops[usize::from(e)], to);
            if !access_ok(world, w2) {
                continue;
            }
            let total = arrive + w2 - t;
            if best.is_none_or(|b| total < b.total) {
                best = Some(Route { board: s, alight: e, hop, depart, arrive, total, walk: 0, dodge: false });
            }
        }
    }
    best
}

/// The island fix round F1 (OPEN-188, `[routes] access_cap_ticks`): a walk
/// to the board stop or from the alight stop this long is no part of a
/// route (V8: a 2.5 h walk to the platform). Always true off, or with the
/// cap 0 (the shipped value: see `[routes]`).
fn access_ok(world: &World, walk: Tick) -> bool {
    world.config.routes.as_ref().is_none_or(|r| r.access_cap_ticks == 0 || walk <= r.access_cap_ticks)
}

/// The island fix round F1 (OPEN-188, `[routes] pass_slack_tiles`): does the
/// walk from `from` to board stop `s` pass alight stop `e` (it lies within
/// the slack of a shortest Manhattan way)? The rider would walk by his stop
/// to ride round to it. Always false off.
pub(super) fn passes(world: &World, from: TilePos, s: u16, e: u16) -> bool {
    let Some(r) = world.config.routes.as_ref() else { return false };
    let net = &world.transit.net;
    let (sp, ep) = (net.stops[usize::from(s)], net.stops[usize::from(e)]);
    from.manhattan(ep) + ep.manhattan(sp) <= from.manhattan(sp) + r.pass_slack_tiles
}

/// The coins a meal costs (the poor test).
fn meal_price(world: &World) -> i64 {
    world.mean_price().max(1)
}

/// T10: how `who` would board now: `Some(false)` pays the fare,
/// `Some(true)` jumps the gate, `None` can't pay and won't jump it. Phase 3:
/// fresh from the cells, a rider with the fare pays it. Pure.
pub fn pays_or_dodges(world: &World, who: EntityId) -> Option<bool> {
    // Pressure pass round 2, R1: banned from the El he walks, unless he
    // boards anyway (the ban broken, never paid: he has no right to the ride).
    let wary = crate::systems::fares::wary(world, who);
    if crate::systems::fares::banned(world, who) {
        return (crate::systems::fares::breaks_ban(world, who) && !wary).then_some(true);
    }
    let c = &world.config.transit;
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    // R1: caught lately, he pays or walks.
    if wary {
        return (coins >= c.fare).then_some(false);
    }
    let law = world.comp::<Personality>(who).map_or(0.5, |p| p.lawfulness);
    let since = world.tick.saturating_sub(u64::from(c.dodge_deterred_days) * TICKS_PER_DAY);
    // R1: caught at the gate lately (the fine, the beating) deters as the
    // cells did.
    let deterred = coins >= c.fare
        && (crate::systems::fares::caught_since(world, who, since)
            || world.comp::<crate::components::Memory>(who).is_some_and(|m| {
                m.entries.iter().any(|e| e.kind == crate::components::MemoryKind::WasArrested && e.tick >= since)
            }));
    // Pressure pass F2: a guard with the fare pays it (the badge at the
    // gate: a commuting guard jumped it and was arrested for his own fare).
    let badge = coins >= c.fare && crate::systems::law::is_guard(world, who);
    let dodge = !deterred && !badge && (law < c.dodge_always || (coins < c.fare && law < c.dodge_lawfulness));
    (dodge || coins >= c.fare).then_some(dodge)
}

/// T8: walk or ride, for `who` from `from` to `to` leaving at `t`. Pure.
/// C9a: `who` would take a ride of `ride` ticks over a walk of `walk` by
/// [`choose`]'s time-vs-fare rule (the fare, or none when he dodges; a coin
/// worth `poor_ticks_per_coin` to the poor), the route aside. `None` when
/// he would not board at all (`pays_or_dodges`).
/// V9: `aboard` of the ride's ticks are on the train (the rest on foot and
/// on the platform, [`felt`]).
pub fn saves_enough(world: &World, who: EntityId, walk: Tick, ride: Tick, aboard: Tick) -> Option<bool> {
    let c = &world.config.transit;
    let dodge = pays_or_dodges(world, who)?;
    let fare = if dodge { 0 } else { c.fare };
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    let poor = coins < c.poor_meals.saturating_mul(meal_price(world));
    let per_coin = if poor { c.poor_ticks_per_coin } else { c.ticks_per_coin };
    let need = u64::from(c.min_saving_ticks) + fare.max(0) as u64 * u64::from(per_coin);
    Some(worth_it(world, walk, ride, aboard, need))
}

/// The view milestone V9 (OPEN-255): a trip of `total` ticks, `aboard` of
/// them on the train, as it feels: the minutes on foot and on the platform
/// weigh `[transit] foot_weight` each (seated, a minute is a minute).
pub fn felt(world: &World, total: Tick, aboard: Tick) -> f64 {
    let foot = total.saturating_sub(aboard) as f64;
    foot * f64::from(world.config.transit.foot_weight) + aboard.min(total) as f64
}

/// The walk-or-ride rule: the ride is no slower than the walk, and the walk
/// feels `need` ticks worse than the ride ([`felt`]).
fn worth_it(world: &World, walk: Tick, ride: Tick, aboard: Tick, need: u64) -> bool {
    walk >= ride && felt(world, walk, 0) - felt(world, ride, aboard) >= need as f64
}

pub fn choose(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Result<Route, Refusal> {
    let c = &world.config.transit;
    if from.manhattan(to) < c.min_ride_tiles {
        return Err(Refusal::Short);
    }
    let mut r = route(world, who, from, to, t).ok_or(Refusal::NoRoute)?;
    let coins = world.comp::<Wallet>(who).map_or(0, |w| w.coins);
    let courage = world.comp::<Personality>(who).map_or(0.5, |p| p.courage);
    let dodge = pays_or_dodges(world, who).ok_or(Refusal::Fare)?;
    let fare = if dodge { 0 } else { c.fare };
    // Time vs fare: a coin is worth more to the poor.
    let poor = coins < c.poor_meals.saturating_mul(meal_price(world));
    let per_coin = if poor { c.poor_ticks_per_coin } else { c.ticks_per_coin };
    let walk = crate::exec::timed_walk(world, who, from, to);
    let need = u64::from(c.min_saving_ticks) + fare.max(0) as u64 * u64::from(per_coin);
    if !worth_it(world, walk, r.total, r.arrive - r.depart, need) {
        return Err(Refusal::Slow);
    }
    // Danger: a night stop in a district past the rider's nerve.
    if is_night(world, r.depart) || is_night(world, r.arrive) {
        let net = &world.transit.net;
        let crime = [r.board, r.alight]
            .iter()
            .map(|&s| world.district(world.district_of(net.stops[usize::from(s)])).crime_rate)
            .fold(0.0f32, f32::max);
        if crime > c.danger_crime * (0.5 + courage) {
            return Err(Refusal::Danger);
        }
    }
    r.walk = walk;
    r.dodge = dodge;
    Ok(r)
}

// ---------------------------------------------------------------------------
// The shared distance and the estimate (phase 2: T16, plan § 6)
// ---------------------------------------------------------------------------

/// The one measure of a commute between two doors, in travel ticks: the
/// walk (Manhattan at the pace, `[exec] walk_ticks`, `life::commute_cost`'s
/// formula), or with the El on the best ride over the doors' nearest stops
/// (`stop_candidates` each, the walks at the same rate, half a `headway`
/// of wait, the table's ride) when it saves `min_saving_ticks` plus a
/// fare's `ticks_per_coin` as it feels (V9: [`felt`]), as `choose` asks of
/// a rider with the coins.
/// No agent, no clock: the structural commute the hiring band, relocation
/// and the housing demand read. God closures count (a closed stop is
/// skipped); a riot's or a curfew's do not (they pass). With transit off it
/// is the walk exactly.
pub fn commute_ticks(world: &World, from: TilePos, to: TilePos) -> Tick {
    commute_parts(world, from, to).0
}

/// V9: [`commute_ticks`] and the ticks of it aboard (0 for the walk).
pub fn commute_parts(world: &World, from: TilePos, to: TilePos) -> (Tick, Tick) {
    let e = &world.config.exec;
    let tiles = from.manhattan(to);
    let walk = e.walk_ticks(Tick::from(tiles));
    let c = &world.config.transit;
    if !c.enabled || tiles < c.min_ride_tiles {
        return (walk, 0);
    }
    let net = &world.transit.net;
    let k = c.stop_candidates.max(1);
    let shut = |s: u16| world.transit.stop_closed.get(&s).is_some_and(|&until| until > world.tick);
    let (mut best, mut aboard) = (UNREACHABLE as Tick, 0);
    let near_to = net.near(to);
    for &s in net.near(from).iter().filter(|&&s| !shut(s)).take(k) {
        let w1 = e.walk_ticks(Tick::from(net.stops[usize::from(s)].manhattan(from)));
        if !access_ok(world, w1) {
            continue;
        }
        for &e in near_to.iter().filter(|&&e| !shut(e)).take(k) {
            if s == e || passes(world, from, s, e) {
                continue;
            }
            let hop = net.hop(s, e);
            if hop.ticks == UNREACHABLE {
                continue;
            }
            let w2 = world.config.exec.walk_ticks(Tick::from(net.stops[usize::from(e)].manhattan(to)));
            if !access_ok(world, w2) {
                continue;
            }
            let ride = w1 + Tick::from(c.headway / 2) + Tick::from(hop.ticks) + w2;
            if ride < best {
                (best, aboard) = (ride, Tick::from(hop.ticks));
            }
        }
    }
    let need = Tick::from(c.min_saving_ticks) + c.fare.max(0) as Tick * Tick::from(c.ticks_per_coin);
    if best < UNREACHABLE as Tick && worth_it(world, walk, best, aboard, need) {
        (best, aboard)
    } else {
        (walk, 0)
    }
}

/// The perf pass phase 3 (the world build): discs `(centre, radius in
/// Manhattan tiles)` whose union holds every `from` with
/// [`commute_ticks`]`(world, from, to) <= limit`: a superset the caller
/// filters by the exact measure, so a query over the discs is the scan of
/// every `from`. The walk's disc around `to`; with the El on, around each
/// stop `s` the tiles from which boarding at `s` could still arrive within
/// `limit`: the ride is at least the half headway, the hop from `s` to an
/// alight stop `commute_ticks` may take (the first `stop_candidates` open
/// ones nearest `to`) and the walk from it, so the walk to `s` is at most
/// what is left (closures, `passes` and the access cap only drop rides).
pub fn reach_discs(world: &World, to: TilePos, limit: Tick) -> Vec<(TilePos, u32)> {
    let e = &world.config.exec;
    let tiles = |t: Tick| u32::try_from(e.walk_tiles(t)).unwrap_or(u32::MAX);
    let mut out = vec![(to, tiles(limit))];
    let c = &world.config.transit;
    if !c.enabled {
        return out;
    }
    // Round 9 Y4: at the real pace a 2 h cap's walk disc holds the whole map
    // from most doors; the stops' discs inside it add nothing but cells read
    // over and over (the seeded staff's graded picks: 17 ms a vacancy at 20k).
    let (w, h) = (world.map.w() as u32, world.map.h() as u32);
    let far = u32::from(to.x).max(w.saturating_sub(1 + u32::from(to.x)))
        + u32::from(to.y).max(h.saturating_sub(1 + u32::from(to.y)));
    if tiles(limit) >= far {
        return out;
    }
    let net = &world.transit.net;
    let k = c.stop_candidates.max(1);
    let shut = |s: u16| world.transit.stop_closed.get(&s).is_some_and(|&until| until > world.tick);
    let alight: Vec<(u16, Tick)> = net
        .near(to)
        .iter()
        .filter(|&&e| !shut(e))
        .take(k)
        .map(|&s| (s, e.walk_ticks(Tick::from(net.stops[usize::from(s)].manhattan(to)))))
        .collect();
    if alight.is_empty() {
        return out;
    }
    let wait = Tick::from(c.headway / 2);
    for s in 0..net.stops.len() {
        let s16 = s as u16;
        let best = alight
            .iter()
            .filter(|&&(e, _)| e != s16)
            .filter_map(|&(e, w2)| {
                let hop = net.hop(s16, e);
                (hop.ticks != UNREACHABLE).then(|| Tick::from(hop.ticks) + w2)
            })
            .min();
        if let Some(rest) = best.map(|b| b + wait).filter(|&b| b <= limit) {
            out.push((net.stops[s], tiles(limit - rest)));
        }
    }
    out
}

/// T14 (phase 3, plan D9): the commute band's upper bounds in ticks: `< 30`,
/// `30-60`, `60-120`, `120-180`, `>= 180` (bands 0-4).
pub const BANDS: [u64; 4] = [30, 60, 120, 180];

/// T14: `id`'s commute band, from its Home door to its work door by
/// `life::commute_cost` (the walk, or the ride where it wins: the same
/// measure on both tiers, read by `calibrate` and `lod::stat_work`). `None`
/// homeless or jobless.
pub fn commute_band(world: &World, id: EntityId) -> Option<usize> {
    let job = world.comp::<crate::components::Job>(id)?;
    let work = world.comp::<crate::components::Building>(job.employer?)?.door;
    let home = crate::systems::contracts::home_door(world, id)?;
    let ticks = crate::systems::life::commute_cost(world, home, work);
    Some(BANDS.iter().position(|&b| ticks < b).unwrap_or(BANDS.len()))
}

/// T16: how long `who` takes from `from` to `to` setting out at `t`: the
/// ride `choose` would take (door to door, its exact wait), else the walk
/// (`exec::timed_walk`). With transit off, or under `min_ride_tiles`, the
/// walk exactly (no route is computed).
pub fn estimate_from(world: &World, who: EntityId, from: TilePos, to: TilePos, t: Tick) -> Tick {
    let walk = crate::exec::timed_walk(world, who, from, to);
    if !may_ride(world, who, from, to) {
        return walk;
    }
    choose(world, who, from, to, t).map_or(walk, |r| r.total.min(walk))
}

/// Could this trip ride at all: the El on and built, the trip past
/// `min_ride_tiles`, no vehicle to hand (a driver never asks, T8).
fn may_ride(world: &World, who: EntityId, from: TilePos, to: TilePos) -> bool {
    enabled(world)
        && from.manhattan(to) >= world.config.transit.min_ride_tiles
        && !world.transit.net.stops.is_empty()
        && !(world.config.assets.enabled && crate::systems::vehicles::vehicle_for_trip(world, who).is_some())
}

/// T16, the departure gate's estimate: the trip to arrive at `to` by
/// `arrive_by`, as `choose` would make it setting out at the time that
/// arrival needs (the structural ride's leaving time, then once more from
/// the leaving time the first answer gives). It reads the deadline, not the
/// clock, so it holds still while a body waits to leave: an estimate read
/// at "now" fell when the El's service began at 05:00 and turned riders
/// back from commutes their walk's gate had opened (the behaviour tier's
/// commute aborts). The walk exactly where `estimate_from` is.
pub fn estimate_by(world: &World, who: EntityId, from: TilePos, to: TilePos, arrive_by: Tick) -> Tick {
    let walk = crate::exec::timed_walk(world, who, from, to);
    if !may_ride(world, who, from, to) {
        return walk;
    }
    let ride = |t: Tick| choose(world, who, from, to, t).map_or(walk, |r| r.total.min(walk));
    let first = ride(arrive_by.saturating_sub(commute_ticks(world, from, to).min(walk)));
    ride(arrive_by.saturating_sub(first))
}

/// T16, the trip gate shared by the departure gate
/// (`routine::must_leave_for_work`, `lead` = its margin) and the shift lead
/// before an outing (`leisure::off_hours`, `lead` = `shift_lead_ticks`):
/// is it time to set out for `door`, `until` ticks off, to arrive `lead`
/// ahead of it? The trip from where `who` stands as it would be made to
/// arrive by then (`estimate_by`: the deadline's trip, not the clock's, so
/// the gate holds still while the body waits). The estimate is never above
/// the walk, so the route is only computed once the walk's gate is open
/// (the departure gate runs every tick for a body resting before its shift).
pub fn trip_due(world: &World, who: EntityId, door: TilePos, until: Tick, lead: Tick) -> bool {
    trip_due_at(world, who, door, until, lead, world.tick)
}

/// [`trip_due`] with the clock at `now` (`until` read from it): the perf
/// pass phase 1's departure gate at a resting body's Rest start.
pub fn trip_due_at(world: &World, who: EntityId, door: TilePos, until: Tick, lead: Tick, now: Tick) -> bool {
    let from = crate::exec::walk_origin(world, who).unwrap_or(door);
    let walk = crate::exec::timed_walk(world, who, from, door);
    if until > walk + lead {
        return false;
    }
    let arrive_by = now + until - lead.min(until);
    until <= estimate_by(world, who, from, door, arrive_by) + lead
}

/// T16: `estimate_from` where `who` stands (outside its building's door),
/// setting out now: the departure gate's and every "how long to get there"
/// reader's (`fixes::walk_estimate`).
pub fn estimate(world: &World, who: EntityId, to: TilePos) -> Tick {
    let from = crate::exec::walk_origin(world, who).unwrap_or(to);
    estimate_from(world, who, from, to, world.tick)
}

// ---------------------------------------------------------------------------
// The legs (T12, T13)
// ---------------------------------------------------------------------------

pub(super) fn count_refusal(world: &mut World, r: Refusal) {
    let c = &mut world.stats.current.transit;
    match r {
        Refusal::Short => {}
        Refusal::NoRoute => c.refused_no_route += 1,
        Refusal::Fare => c.refused_fare += 1,
        Refusal::Slow => c.refused_slow += 1,
        Refusal::Danger => c.refused_danger += 1,
    }
}
