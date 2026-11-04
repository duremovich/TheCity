use super::*;

// ---------------------------------------------------------------------------
// Cabs and rickshaws (§ 3)
// ---------------------------------------------------------------------------

/// A Garage's cab drivers (its `contract` staff on a `fare` duty).
pub(super) fn drivers(world: &World, g: EntityId) -> u32 {
    world
        .staff_of(g)
        .iter()
        .filter(|&&s| {
            world
                .comp::<Job>(s)
                .and_then(|j| crate::systems::trades::row_of(world, j.role))
                .is_some_and(|r| r.duty == TradeDuty::Fare)
        })
        .count() as u32
}

/// `lod::stat_work`, the shift's first hour (before the El's choice): an
/// off-screen commuter of `cab_min_tiles` or more holding the fare and
/// `cab_reserve` takes a ride on `p_cab` (a pure hash of the day and the
/// agent): an open rickshaw of his home district with rides left first
/// (`rickshaw_mult` × the fare to its owner), else the Garage nearest home
/// with cab drivers and rides left (`Flow::CabFare` to its owner). `true`
/// when he rode (the El's leg is skipped).
pub fn stat_cab(world: &mut World, id: EntityId, job: &Job) -> bool {
    if !on(world) {
        return false;
    }
    let tod = world.tick_of_day();
    let first = job.shifts.iter().any(|&(s, e)| tod >= s && tod < e && tod - s < TICKS_PER_HOUR as u16 && s != 0);
    // R6a: the shift's last hour rides home (`cab_roll_home`).
    // A night shift's 24:00 is not its end (it runs on from 00:00).
    let wraps = job.shifts.iter().any(|&(s, _)| s == 0);
    let last = !first
        && job
            .shifts
            .iter()
            .any(|&(s, e)| tod >= s && tod < e && e - tod <= TICKS_PER_HOUR as u16 && !(e == 1440 && wraps));
    if !first && !last {
        return false;
    }
    let ride = if first { cab_roll(world, id, job) } else { cab_roll_home(world, id, job) };
    let Some(ride) = ride else { return false };
    // Lives at work phase 4 (§ 2.3, D3): the ride booked to a driver's
    // body (the same rule as a Full rider's) and its encounter. Nothing off.
    // Round 6 V4: booked first, so the fare goes to the driver.
    let driver = match ride.by {
        RideBy::Cab(g) => book_cab(world, id, g, ride.from, &ride).map(|(s, _)| s),
        RideBy::Rickshaw(_) => None,
    };
    pay_ride(world, id, &ride, driver);
    let d = world.district_of(ride.from);
    ride_encounter(world, id, driver, &ride, d);
    true
}

/// Lives at work phase 4 (§ 2.3): the cab roll, one source for both tiers
/// (a Full commuter rolls the same hash and gets the same pick as a
/// Statistical one; the coins move in `pay_ride`). The roll `stat_cab`
/// made inline until phase 4, unchanged: `p_cab` on a hash of the agent
/// and the day, `cab_min_tiles`, the fare, a working rickshaw of the home
/// district with rides left first (its fare, or no ride when the purse is
/// short), else the Garage nearest home with drivers and rides left.
pub fn cab_roll(world: &World, id: EntityId, job: &Job) -> Option<Ride> {
    roll(world, id, job, false)
}

/// The pressure pass R6a (OPEN-217): the commute's roll the other way, the
/// ride home off a shift (`stat_cab`'s last hour, `full_cab`'s GoTo home
/// from the workplace): the same rule (the purse, `cab_min_tiles`, the El
/// not shortening it or the `p_cab` roll on its own hash), picked up at the
/// work door. The commute's cab rode only in: every rider came at a shift's
/// start, so an evening driver (16-24) stood by through no rush at all.
/// `None` with `[trade_verbs]` absent.
pub fn cab_roll_home(world: &World, id: EntityId, job: &Job) -> Option<Ride> {
    world.config.trade_verbs.as_ref()?;
    roll(world, id, job, true)
}

pub(super) fn roll(world: &World, id: EntityId, job: &Job, home_bound: bool) -> Option<Ride> {
    if !on(world) {
        return None;
    }
    let cfg = &world.config.jobs2;
    let day_key = if home_bound { world.day() ^ PURPOSE_CAB_HOME } else { world.day() };
    let rolled = hash(world, PURPOSE_CAB, u64::from(id.index), day_key) < cfg.p_cab;
    let el_missed = world.config.legs.as_ref().is_some_and(|l| l.fares_el_missed);
    if !rolled && !el_missed {
        return None;
    }
    let work = job.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door)?;
    let home = crate::systems::contracts::home_door(world, id)?;
    let tiles = home.manhattan(work);
    if tiles < cfg.cab_min_tiles {
        return None;
    }
    // The pre-view pass phase 5 (the `fares` shape): a commute the El does
    // not shorten (`transit::commute_ticks` is the walk) takes the cab
    // without the roll, both tiers (the one source).
    if !rolled {
        let walk = world.config.exec.walk_ticks(Tick::from(tiles));
        if crate::systems::transit::commute_ticks(world, home, work) < walk {
            return None;
        }
    }
    let fare = cfg.fare_base + (tiles as f32 * cfg.fare_per_tile).round() as i64;
    let purse = coins(world, id);
    // Round 6 V4 (OPEN-240): the commute reads the wage: a fare past
    // `cab_wage_share` of the day's pay walks or rides the El, unless flush
    // (`fabber_84414`: 7c a leg on a 7c wage, net -12c a day).
    if cfg.cab_wage_share > 0.0
        && purse < cfg.cab_flush_coins
        && fare as f32 > cfg.cab_wage_share * job.wage_per_day.max(0) as f32
    {
        return None;
    }
    let d = world.district_of(home);
    let rickshaw = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.district == d && p.owner != id && u32::from(p.jobs_today) < cfg.rides_per_rickshaw)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Rickshaw) && working(world, p))
        .map(|p| p.owner)
        .next();
    if let Some(r) = rickshaw {
        let f = ((fare as f32 * cfg.rickshaw_mult).round() as i64).max(1);
        if purse < f + cfg.cab_reserve {
            return None;
        }
        let (from, to) = if home_bound { (work, home) } else { (home, work) };
        return Some(Ride { from, to, fare: f, by: RideBy::Rickshaw(r) });
    }
    if purse < fare + cfg.cab_reserve {
        return None;
    }
    let (from, to) = if home_bound { (work, home) } else { (home, work) };
    let open = open_garages(world);
    let garage = if world.config.trade_verbs.is_some() {
        garage_for(world, &open, from, id)?
    } else {
        open.iter()
            .copied()
            .min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(from)), g))?
    };
    Some(Ride { from, to, fare, by: RideBy::Cab(garage) })
}

/// The open Garages with cab drivers and rides left today.
fn open_garages(world: &World) -> Vec<EntityId> {
    let per = world.config.jobs2.rides_per_driver;
    world
        .buildings_of_kind(BuildingKind::Garage)
        .iter()
        .copied()
        .filter(|&g| crate::systems::industry::open(world, g))
        .filter(|&g| world.jobs2.cab_trips.get(&g).copied().unwrap_or(0) < drivers(world, g) * per)
        .collect()
}

/// The ticks a rider waits at the door for a driver's body.
fn fare_wait(world: &World) -> Tick {
    match world.config.legs.as_ref() {
        Some(l) => Tick::from(l.fare_wait_ticks),
        None => Tick::from(world.config.service.cab_wait_ticks),
    }
}

/// R6a: a driver's fares this shift (`fares_shift`).
pub(super) fn fares_of(world: &World, s: EntityId) -> u16 {
    world.jobs2.fares_shift.get(&s).copied().unwrap_or(0)
}

/// The pressure pass R6a (OPEN-217): the wait for `rider`: an off-screen
/// rider's is `[trade_verbs] stat_fare_wait_ticks` (nobody sees him at his
/// door; a Garage keeps one driver a shift, so at the body's wait most
/// rides fell to the off-screen cab: 20k, 3 days, 240 bookings with no
/// driver on shift and most of the rest with the one driver an hour or more
/// away), a body's `fare_wait`.
fn fare_wait_for(world: &World, rider: EntityId) -> Tick {
    match world.config.trade_verbs.as_ref() {
        Some(c) if world.comp::<Brain>(rider).is_none_or(|b| b.lod == Lod::Statistical) => {
            c.stat_fare_wait_ticks.max(fare_wait(world))
        }
        _ => fare_wait(world),
    }
}

/// The pressure pass R6a (OPEN-217): the Garage a ride picked up at `at`
/// goes to: the one whose standing-by driver's body can come soonest
/// within the rider's wait (any Garage), else the nearest (its off-screen
/// cab). The commute's roll took the Garage nearest the rider's Home, so a
/// followed driver whose Garage was not the nearest drove nobody while the
/// nearest Garage's off-screen cab took the fare (20k seed 42 x 30: 396
/// rides a body drove, 3,637 unmanned; `fares_no_body` 2,096).
fn garage_for(world: &World, open: &[EntityId], at: TilePos, rider: EntityId) -> Option<EntityId> {
    let wait = fare_wait_for(world, rider);
    // The dispatch: the free body within the wait with the fewest fares this
    // shift, then the soonest (a rank's turn: the soonest alone gave the
    // driver nearest the riders every fare and his neighbours none).
    // The day's cap (`rides_per_driver`) is the off-screen cab's: a Garage
    // past it still sends its driver's body when he stands by (the cab's
    // unmanned rides filled a busy Garage's cap by the morning rush, and its
    // driver's body stood by for the rest of the shift with nothing booked).
    let all: Vec<EntityId> = if world.config.trade_verbs.is_some() {
        world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .copied()
            .filter(|&g| crate::systems::industry::open(world, g))
            .collect()
    } else {
        open.to_vec()
    };
    let body = all
        .iter()
        .filter_map(|&g| {
            drivers_free(world, g, at)
                .into_iter()
                .filter(|&(_, eta)| eta <= wait)
                .map(|(s, eta)| (if world.config.trade_verbs.is_some() { fares_of(world, s) } else { 0 }, eta, g))
                .min()
        })
        .min()
        .map(|(_, _, g)| g);
    body.or_else(|| {
        open.iter().copied().min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(at)), g))
    })
}

/// Lives at work phase 4 (§ 2.3): the ride's coins and counters (the
/// rickshaw's or the Garage's `Flow::CabFare`, the pitch's or the Garage's
/// rides today). Returns the coins paid. Round 6 V4 (OPEN-240): the rider
/// pays the cab: `driver` (the body who drove him), else the Garage's
/// off-screen driver of record ([`driver_of_record`]), takes the fare and
/// hands the house its share (`Flow::CabTake`): a body keeps `[verbs]
/// cab_fare_cut` (his pay, `driver_cut`), an off-screen driver (on the
/// mirror's wage) hands it all. No driver at all: the owner, as before.
pub fn pay_ride(world: &mut World, id: EntityId, ride: &Ride, driver: Option<EntityId>) -> i64 {
    match ride.by {
        RideBy::Rickshaw(r) => {
            let paid = ownership::pay(world, Some(id), Some(r), ride.fare, Flow::CabFare);
            if let Some(p) = world.jobs2.pitches.get_mut(&r) {
                p.jobs_today = p.jobs_today.saturating_add(1);
                p.take_today += paid;
            }
            let row = ownership::jobs2_row(world);
            row.rickshaw_trips += 1;
            row.rickshaw_fares += paid;
            paid
        }
        RideBy::Cab(g) => {
            let owner = world.owner_of(g);
            let body = driver.filter(|&s| s != id && world.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical));
            let cabbie = body.or_else(|| driver_of_record(world, g).filter(|&s| s != id));
            let paid = match cabbie {
                Some(s) => {
                    let before = world.purse(Some(s));
                    let paid = ownership::pay(world, Some(id), Some(s), ride.fare, Flow::CabFare);
                    // What reached his hand (the fare's tax withheld at him).
                    let got = (world.purse(Some(s)) - before).max(0);
                    let keep = if body.is_some() { driver_cut(world, s, got) } else { 0 };
                    ownership::pay(world, Some(s), owner, got - keep, Flow::CabTake);
                    paid
                }
                None => ownership::pay(world, Some(id), owner, ride.fare, Flow::CabFare),
            };
            ownership::credit(world, g, paid);
            *world.jobs2.cab_trips.entry(g).or_default() += 1;
            let row = ownership::jobs2_row(world);
            row.cab_trips += 1;
            row.cab_fares += paid;
            paid
        }
    }
}

// ---------------------------------------------------------------------------
// Lives at work phase 4 (§ 2.3): the Full commuter's cab and the driver's body
// ---------------------------------------------------------------------------

/// The ticks a cab takes from `from` to `to`: the walk's ticks × the Car's
/// `timed_mult` (`[vehicles] mult`), at least one.
pub fn cab_ticks(world: &World, from: TilePos, to: TilePos) -> Tick {
    let m = crate::systems::vehicles::timed_mult_of(world, crate::components::AssetKind::Car);
    world.config.exec.walk_ticks_f(Tick::from(from.manhattan(to)), m).max(1)
}

/// The on-duty cab drivers of `garage` with a body (Full or Coarse) and no
/// ride pending, with the ticks each needs to reach `at`.
pub(super) fn drivers_free(world: &World, garage: EntityId, at: TilePos) -> Vec<(EntityId, Tick)> {
    world
        .staff_of(garage)
        .iter()
        .copied()
        .filter(|&s| {
            standing_by(world, s)
                && !world.has::<Sentence>(s)
                && crate::systems::law::living(world, s)
                && world.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical)
        })
        .filter_map(|s| {
            let from = crate::exec::walk_origin(world, s)?;
            Some((s, cab_ticks(world, from, at)))
        })
        .collect()
}

/// `exec::start_step`, a Full or Coarse commuter's `GoTo` to its workplace
/// (before the vehicle, the El and the walk): the cab roll shared with the
/// Statistical tier, once a day a rider (`Jobs2State.cab_rolled`). A ride
/// rolled: the fare paid (`pay_ride`; a rider `transit::pays_or_dodges`
/// says dodges rides `Short`: no coins, a `FareShort` event, the driver's
/// grudge), the rider's leg timed at the cab's speed, and the Garage's
/// on-duty driver with a body who can reach the rider within
/// `cab_wait_ticks` and the lowest hash (D6) takes the ride: a `CabRide`
/// his body drives (`fare_plan`), the `Fare` encounter at the pick-up.
/// With no such driver the Garage's off-screen cab takes it (deviation:
/// the rider never waits, the shift matters more than the ride). Nothing
/// off.
pub fn full_cab(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    // Review fix 10: the cheap checks on a borrow; the Job cloned only for the roll.
    let job = world.comp::<Job>(id)?;
    // R6a (OPEN-217): off a shift, the GoTo home from the workplace is the
    // ride home's roll, once a day (`exec_home_rolled`: an exec holds no Job,
    // so the set is free for a worker's ride home).
    if world.config.trade_verbs.is_some()
        && target.building.is_some()
        && target.building == crate::systems::missions::home_of(world, id)
        && job.employer.is_some()
        && world.comp::<Position>(id).is_some_and(|p| p.building == job.employer)
        && !job.on_shift(world.tick_of_day())
        && !world.jobs2.exec_home_rolled.contains(&id)
    {
        let job = job.clone();
        world.jobs2.exec_home_rolled.insert(id);
        let ride = cab_roll_home(world, id, &job)?;
        let here = crate::exec::walk_origin(world, id)?;
        return take_ride(world, id, target, ride, here);
    }
    if target.building.is_none() || target.building != job.employer || world.jobs2.cab_rolled.contains(&id) {
        return None;
    }
    let job = job.clone();
    world.jobs2.cab_rolled.insert(id);
    let ride = cab_roll(world, id, &job)?;
    let here = crate::exec::walk_origin(world, id)?;
    take_ride(world, id, target, ride, here)
}

/// The pressure pass phase 2 (OPEN-176, `[customers]`): a body's GoTo to a
/// building `hail_min_tiles` or more away, not to work (the commute's roll
/// is `full_cab`'s), hails a cab once a day when rich (`hail_rich_coins`)
/// or hurried (its goal is `Treat`, `Flee` or `Rescue`, or a shift starting
/// within the walk), holding the fare and `cab_reserve`, on `p_hail` (a
/// hash of the rider and the day): the Garage nearest the rider with
/// drivers and rides left sends a driver's body (`book_cab`, the pick-up
/// and the drop-off legs) or its off-screen cab, as the commuter's ride.
pub fn hail_cab(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    let c = crate::systems::customers::cfg(world)?.clone();
    if !crate::systems::service::street_on(world) || world.jobs2.hailed.contains(&id) {
        return None;
    }
    let to_b = target.building?;
    let here = crate::exec::walk_origin(world, id)?;
    let to = world.comp::<Building>(to_b)?.door;
    let tiles = here.manhattan(to);
    if tiles < c.hail_min_tiles {
        return None;
    }
    // Late for a shift: the walk is longer than the time left before it.
    let walk = world.config.exec.walk_ticks(Tick::from(tiles));
    let late = world
        .comp::<Job>(id)
        .is_some_and(|j| j.employer == Some(to_b) && Tick::from(j.ticks_until_shift(world.tick_of_day())) < walk);
    if world.comp::<Job>(id).is_some_and(|j| j.employer == Some(to_b)) && !late {
        return None;
    }
    let cfg = &world.config.jobs2;
    let fare = cfg.fare_base + (tiles as f32 * cfg.fare_per_tile).round() as i64;
    let purse = coins(world, id);
    if purse < fare + cfg.cab_reserve {
        return None;
    }
    let goal = world.comp::<Brain>(id).and_then(|b| b.current_goal);
    let hurried = late || matches!(goal, Some(GoalKind::Treat | GoalKind::Flee | GoalKind::Rescue));
    if purse < c.hail_rich_coins && !hurried {
        return None;
    }
    if hash(world, PURPOSE_HAIL, u64::from(id.index), world.day()) >= c.p_hail {
        return None;
    }
    // A hailed cab is the one a driver's body can bring soonest within the
    // rider's wait, from any Garage; else the nearest Garage's (off screen).
    let open = open_garages(world);
    let garage = garage_for(world, &open, here, id)?;
    world.jobs2.hailed.insert(id);
    ownership::jobs2_row(world).cab_hails += 1;
    let why = if late {
        "late for a shift"
    } else if hurried {
        "in a hurry"
    } else {
        "flush"
    };
    let text = format!(
        "{} hailed a cab ({why}) from {} to {}",
        world.name_of(id),
        world.district_name(world.district_of(here)),
        world.name_of(to_b)
    );
    world.push_event(EventKind::Served, &[id, garage, to_b], text);
    let ride = Ride { from: here, to, fare, by: RideBy::Cab(garage) };
    take_ride(world, id, target, ride, here)
}

/// A ride taken now from `here` (the commuter's and the hailer's): the
/// driver booked, the fare paid (or dodged), the leg timed or carried.
fn take_ride(
    world: &mut World,
    id: EntityId,
    target: &crate::exec::GotoTarget,
    ride: Ride,
    here: TilePos,
) -> Option<crate::exec::ExecState> {
    let now = world.tick;
    // `pays_or_dodges` is `Some(dodge)`: `Some(true)` rides without paying.
    let dodges = crate::systems::transit::pays_or_dodges(world, id) == Some(true);
    let mut arrive = now + cab_ticks(world, here, ride.to);
    let driver = match ride.by {
        RideBy::Cab(garage) => book_cab(world, id, garage, here, &ride),
        RideBy::Rickshaw(_) => None,
    };
    if let Some((_, eta)) = driver {
        arrive += eta;
    }
    // The pre-view pass phase 5 (the `fares` shape, OPEN-115): a driver's
    // body comes for him: he waits at his door and rides along (`carry`),
    // the fare paid on arrival and the encounter at the pick-up
    // (`fare_plan`). A dodger's short fare is as before.
    if let (Some((s, _)), RideBy::Cab(g)) = (driver.filter(|_| fares_on(world)), ride.by) {
        if let Some(r) = world.jobs2.cab_rides.iter_mut().rev().find(|r| r.rider == id && r.driver == s) {
            r.body = true;
            if dodges {
                r.fare = 0;
            }
        }
        if dodges {
            ownership::jobs2_row(world).fares_short += 1;
            let (who, at) = (world.name_of(id), world.district_name(world.district_of(here)).to_string());
            let w = world.config.service.tune.fare_dodge_grudge;
            crate::systems::grudges::add(world, s, id, crate::word::GrudgeCause::Robbed, w, 0);
            let text = format!("{who} rode {}'s cab from {at} and skipped the {} fare", world.name_of(s), ride.fare);
            world.push_event(EventKind::FareShort, &[id, s, g], text);
        }
        world.jobs2.passengers.insert(id, s);
        world.jobs2.riding.insert(id, Tick::MAX);
        return Some(crate::exec::ExecState::GotoTimed {
            target: target.clone(),
            arrive_tick: Tick::MAX,
            blocked_since: None,
            walk: None,
            climb: None,
        });
    }
    if fares_on(world) {
        ownership::jobs2_row(world).fares_unmanned += 1;
    }
    if dodges {
        ownership::jobs2_row(world).fares_short += 1;
        let (who, at) = (world.name_of(id), world.district_name(world.district_of(here)).to_string());
        match (driver, ride.by) {
            (Some((s, _)), RideBy::Cab(g)) => {
                let w = world.config.service.tune.fare_dodge_grudge;
                crate::systems::grudges::add(world, s, id, crate::word::GrudgeCause::Robbed, w, 0);
                let text =
                    format!("{who} rode {}'s cab from {at} and skipped the {} fare", world.name_of(s), ride.fare);
                world.push_event(EventKind::FareShort, &[id, s, g], text);
            }
            (_, RideBy::Cab(g)) => {
                let text = format!("{who} skipped a {} cab fare from {at}", ride.fare);
                world.push_event(EventKind::FareShort, &[id, g], text);
            }
            (_, RideBy::Rickshaw(r)) => {
                let text = format!("{who} skipped {}'s {} rickshaw fare from {at}", world.name_of(r), ride.fare);
                world.push_event(EventKind::FareShort, &[id, r], text);
            }
        }
    } else {
        pay_ride(world, id, &ride, driver.map(|(s, _)| s));
    }
    let d = world.district_of(here);
    ride_encounter(world, id, driver.map(|(s, _)| s), &ride, d);
    world.jobs2.riding.insert(id, arrive);
    Some(crate::exec::ExecState::GotoTimed {
        target: target.clone(),
        arrive_tick: arrive,
        blocked_since: None,
        walk: None,
        climb: None,
    })
}

/// 8d (OPEN-129, Dylan 2026-10-19): `exec::start_step`, an exec's `GoTo`
/// to his corp's Hq (`life::hq_of`), before the vehicle, the El and the
/// walk. With `[service] exec_ride`, the street on and the exec housed at
/// `[life] exec_home_tier` or better (the Spire) in a district other than
/// the Hq's (with `[routes]`, the island fix round F1: `exec_ride_min_tiles`
/// or more from it, a label not being space): the leg is timed at the Car's speed (`cab_ticks`, 4a's cab
/// leg). The corp's own road vehicle drives him when it owns one (no fare;
/// deviation: the vehicle's asset is not moved, as the off-screen cab's
/// is not), else the Garage nearest his Home with rides left sends its
/// off-screen cab and the corp pays the fare (`Flow::CabFare`, the
/// Garage's `cab_trips`). No Garage, or a corp short of the fare: he
/// walks. Once a day (`cab_rolled`). Both tiers: a Statistical exec's
/// office trip is the same `GoTo` (timed), so the mirror is the rule
/// itself. Nothing off.
pub fn exec_ride(world: &mut World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    if !crate::systems::service::street_on(world) || !world.config.service.exec_ride {
        return None;
    }
    let tier = crate::systems::life::exec_home_tier(world);
    if tier == 0 || world.has::<Job>(id) {
        return None;
    }
    let to = target.building?;
    // The pre-view pass phase 5 (OPEN-140's remainder): the ride home, from
    // anywhere outside the Home's district, once a day (`exec_home_rolled`).
    let home_ride = world.config.legs.as_ref().is_some_and(|l| l.exec_ride_home)
        && crate::systems::missions::home_of(world, id) == Some(to);
    if home_ride {
        if world.jobs2.exec_home_rolled.contains(&id) {
            return None;
        }
    } else {
        if world.jobs2.cab_rolled.contains(&id) {
            return None;
        }
        // Review fix 10: a GoTo to a corp's building whose exec he is, before the
        // scan of every corp (`Corp.buildings` and `Building.owner` agree:
        // `ownership::transfer_building`).
        let owner = world.corp_of_building(to)?;
        if world.comp::<Corp>(owner).and_then(|c| c.exec) != Some(id) {
            return None;
        }
    }
    let corp = crate::systems::life::exec_corp(world, id)?;
    let hq = crate::systems::life::office_of(world, corp, id)?;
    if !home_ride && target.building != Some(hq) {
        return None;
    }
    let home = crate::systems::missions::home_of(world, id)?;
    let hb = world.comp::<Building>(home)?;
    // The pressure pass F1: an exec not yet housed at the tier (no free
    // Spire flat near his Hq: `life::house_exec`) is driven too; the tier
    // gate walked a CEO 764 tiles across the Mid-Levels to his Lab.
    if near_by_label(world, home, hb.door, hq) {
        return None;
    }
    let home_door = hb.door;
    let here = crate::exec::walk_origin(world, id)?;
    // The pressure pass F1: the car is timed at its Coarse road share (0.88
    // min a tile against the El's 0.25), so across the island it is slower
    // than the train: a ride the El beats is not taken, and the GoTo rides
    // the El (`transit::try_ride`) or walks. A Spire exec sat 601 min in a
    // car to his Sump Hq.
    if crate::systems::transit::commute_ticks(world, here, target.tile) < cab_ticks(world, here, target.tile) {
        return None;
    }
    if home_ride {
        if near_home(world, here, home, home_door) {
            return None;
        }
        world.jobs2.exec_home_rolled.insert(id);
    } else {
        world.jobs2.cab_rolled.insert(id);
    }
    let road = |x: &crate::components::Asset| {
        x.kind.is_road_vehicle() && x.kind != crate::components::AssetKind::Truck && x.condition > 0 && !x.stolen
    };
    let corp_car = crate::systems::assets::assets_of(world, Some(corp))
        .iter()
        .any(|&v| world.comp::<crate::components::Asset>(v).is_some_and(road));
    if !corp_car {
        let cfg = &world.config.jobs2;
        let fare = cfg.fare_base + (here.manhattan(target.tile) as f32 * cfg.fare_per_tile).round() as i64;
        let garage = world
            .buildings_of_kind(BuildingKind::Garage)
            .iter()
            .copied()
            .filter(|&g| crate::systems::industry::open(world, g))
            .filter(|&g| world.jobs2.cab_trips.get(&g).copied().unwrap_or(0) < drivers(world, g) * cfg.rides_per_driver)
            .min_by_key(|&g| (world.comp::<Building>(g).map_or(u32::MAX, |b| b.door.manhattan(home_door)), g))?;
        if world.purse(Some(corp)) < fare {
            return None;
        }
        let owner = world.owner_of(garage);
        let paid = ownership::pay(world, Some(corp), owner, fare, Flow::CabFare);
        ownership::credit(world, garage, paid);
        *world.jobs2.cab_trips.entry(garage).or_default() += 1;
        let row = ownership::jobs2_row(world);
        row.cab_trips += 1;
        row.cab_fares += paid;
        row.cab_rides_unmanned += 1;
    }
    let row = ownership::jobs2_row(world);
    row.exec_rides += 1;
    if corp_car {
        row.exec_rides_corp += 1;
    }
    if home_ride {
        row.exec_rides_home += 1;
    }
    let arrive = world.tick + cab_ticks(world, here, target.tile);
    Some(crate::exec::ExecState::GotoTimed {
        target: target.clone(),
        arrive_tick: arrive,
        blocked_since: None,
        walk: None,
        climb: None,
    })
}

/// The pre-view pass phase 5 (OPEN-140's remainder): the ticks of the car
/// ride home `exec_ride` would give `id` from `from` to his Home's `door`
/// (an exec housed at `exec_home_tier` whose Hq stands in another district,
/// `from` outside the Home's district), the Car's speed; `None` otherwise or
/// with `[legs] exec_ride_home` off. Pure (the fare and the Garage are the
/// ride's, not the bed's).
pub fn ride_home_ticks(world: &World, id: EntityId, from: TilePos, door: TilePos) -> Option<Tick> {
    if !crate::systems::service::street_on(world)
        || !world.config.service.exec_ride
        || !world.config.legs.as_ref().is_some_and(|l| l.exec_ride_home)
    {
        return None;
    }
    let tier = crate::systems::life::exec_home_tier(world);
    if tier == 0 || world.has::<Job>(id) {
        return None;
    }
    let corp = crate::systems::life::exec_corp(world, id)?;
    let hq = crate::systems::life::office_of(world, corp, id)?;
    let home = crate::systems::missions::home_of(world, id)?;
    let hb = world.comp::<Building>(home)?;
    if near_by_label(world, home, hb.door, hq) || near_home(world, from, home, hb.door) {
        return None;
    }
    // The pressure pass F1: as `exec_ride`, no car ride the El beats.
    let car = cab_ticks(world, from, door);
    (crate::systems::transit::commute_ticks(world, from, door) >= car).then_some(car)
}

/// The exec's car's test of a Home and an Hq too near to drive: with
/// `[routes]` (the island fix round F1, OPEN-188), under `exec_ride_min_tiles`
/// door to door; off, the base's one district label (a Spire CEO 300 tiles
/// from his Spire Hq walked).
fn near_by_label(world: &World, home: EntityId, home_door: TilePos, hq: EntityId) -> bool {
    match world.config.routes.as_ref() {
        Some(r) => world.comp::<Building>(hq).is_none_or(|b| b.door.manhattan(home_door) < r.exec_ride_min_tiles),
        None => world.district_of_building(home) == world.district_of_building(hq),
    }
}

/// The ride home's test of `from` too near the Home: with `[routes]`, under
/// `exec_ride_min_tiles` of its door; off, inside its district.
fn near_home(world: &World, from: TilePos, home: EntityId, home_door: TilePos) -> bool {
    match world.config.routes.as_ref() {
        Some(r) => from.manhattan(home_door) < r.exec_ride_min_tiles,
        None => world.district_of(from) == world.district_of_building(home),
    }
}

/// Lives at work phase 4 (§ 2.3, D3, D6): a cab ride from `garage` booked
/// to a driver's body: the standing-by driver who can reach `from` within
/// `cab_wait_ticks` at cab speed with the lowest `hash(PURPOSE_FARE, rider,
/// driver)` takes a `CabRide` (`cab_rides_driven`); none, the Garage's
/// off-screen cab (`cab_rides_unmanned`). Both tiers' riders (`full_cab`,
/// `stat_cab`): the ride is world-side, the driver's body animates it.
/// Returns the driver and his ticks to the pick-up. Nothing off.
pub(super) fn book_cab(
    world: &mut World,
    rider: EntityId,
    garage: EntityId,
    from: TilePos,
    ride: &Ride,
) -> Option<(EntityId, Tick)> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    let wait = fare_wait_for(world, rider);
    let free = drivers_free(world, garage, from);
    // The pre-view pass phase 5 (OPEN-115's diagnosis): why no body drives.
    if fares_on(world) && !free.iter().any(|&(_, eta)| eta <= wait) {
        let row = ownership::jobs2_row(world);
        if free.is_empty() {
            row.fares_no_body += 1;
        } else {
            row.fares_too_far += 1;
        }
    }
    // R6a: the fewest fares this shift first (`garage_for`'s dispatch), then the hash.
    let fewest = world.config.trade_verbs.is_some();
    let driver = free
        .into_iter()
        .filter(|&(_, eta)| eta <= wait)
        .map(|(s, eta)| {
            let n = if fewest { fares_of(world, s) } else { 0 };
            (n, hash(world, PURPOSE_FARE, u64::from(rider.index), u64::from(s.index)).to_bits(), s, eta)
        })
        .min()
        .map(|(_, _, s, eta)| (s, eta));
    match driver {
        Some((s, _)) => {
            let (to, fare, tick) = (ride.to, ride.fare, world.tick);
            world.jobs2.cab_rides.push(CabRide { rider, from, to, driver: s, fare, tick, stage: 0, body: false });
            ownership::jobs2_row(world).cab_rides_driven += 1;
        }
        None => ownership::jobs2_row(world).cab_rides_unmanned += 1,
    }
    driver
}
