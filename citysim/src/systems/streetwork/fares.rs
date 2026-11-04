use super::*;

/// Lives at work phase 4 (§ 2.3): the ride's `Fare` encounter at the
/// pick-up: the booked driver's at the Garage, the rickshaw puller's in the
/// rider's district (his pitch is a tile); an unmanned cab, none. Nothing off.
pub(super) fn ride_encounter(world: &mut World, rider: EntityId, driver: Option<EntityId>, ride: &Ride, d: DistrictId) {
    if !crate::systems::service::street_on(world) {
        return;
    }
    let kind = crate::components::ServeKind::Fare;
    match (driver, ride.by) {
        (Some(s), RideBy::Cab(g)) => {
            *world.jobs2.fares_shift.entry(s).or_default() += 1;
            crate::systems::service::serve(world, s, rider, g, kind, 0)
        }
        (_, RideBy::Rickshaw(r)) => crate::systems::service::encounter(world, r, rider, None, d, kind, 0),
        _ => {}
    }
}

/// Round 3 S5 (OPEN-217): the driver's body keeps `[verbs] cab_fare_cut`
/// of the `fare` he took in hand, told in his diary as the shift's line
/// (`fare_cuts`) and tallied (`cab_cuts`); returns the coins kept. A
/// dodged fare (0) keeps nothing. The fare went to the corp and the
/// shift's wage, scaled by the fares, paid 0c and wrote "Paid" on a
/// fare-less shift (`cabbie_82150`, 0c all week). Round 6 V4 (OPEN-240):
/// the rider pays him (`pay_ride`) and he hands the rest in; U7's cut came
/// from the owner, who had the fare.
pub(super) fn driver_cut(world: &mut World, s: EntityId, fare: i64) -> i64 {
    let Some(share) = world.config.verbs.as_ref().map(|v| v.cab_fare_cut).filter(|&c| c > 0.0) else { return 0 };
    let cut = ((fare as f32 * share).round() as i64).clamp(0, fare.max(0));
    if cut > 0 {
        *world.jobs2.fare_cuts.entry(s).or_default() += cut;
        ownership::jobs2_row(world).cab_cuts += cut;
    }
    cut
}

/// Round 6 V4 (OPEN-240): the off-screen cab's driver, who takes an
/// unmanned ride's fare for Garage `g`: a living, free Statistical driver
/// on its staff, on shift first, then the fewest fares today, then the
/// lowest id. `None`: the Garage has none (the owner takes the fare).
pub(super) fn driver_of_record(world: &World, g: EntityId) -> Option<EntityId> {
    let tod = world.tick_of_day();
    world
        .staff_of(g)
        .iter()
        .copied()
        .filter(|&s| {
            world.comp::<Job>(s).is_some_and(|j| fare_duty(world, j))
                && crate::systems::law::living(world, s)
                && !world.has::<Sentence>(s)
                && world.comp::<Brain>(s).is_some_and(|b| b.lod == Lod::Statistical)
        })
        .min_by_key(|&s| (!world.comp::<Job>(s).is_some_and(|j| j.on_shift(tod)), fares_of(world, s), s))
}

/// The ride `id` drives now, if any (the first pending with him as
/// driver, while he can still drive it: [`can_drive`]).
fn ride_of(world: &World, id: EntityId) -> Option<usize> {
    if !can_drive(world, id) {
        return None;
    }
    world.jobs2.cab_rides.iter().position(|r| r.driver == id)
}

/// Review fix 2: `id` can still drive a booked ride: alive, free, a cab
/// driver's Job (a `fare` duty) on its shift. A ride whose driver cannot is
/// not his ([`ride_of`]) and the hour drops it ([`drop_stale_rides`]); the
/// rider's leg was timed at the booking, so he arrives all the same (the
/// off-screen cab's ride).
fn can_drive(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Job>(id).is_some_and(|j| fare_duty(world, j) && j.on_shift(world.tick_of_day()))
        // The pre-view pass phase 5: a body drives it (a driver gone off
        // screen leaves his rider to the off-screen cab).
        && (!fares_on(world) || world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical))
}

/// Review fix 2 (`hourly`): drop the booked rides whose driver can no
/// longer drive them (the shift over, the post lost, jailed or dead), and
/// (C9a) a body ride whose driver has not come in `cab_no_show_ticks`.
pub(super) fn drop_stale_rides(world: &mut World) {
    if world.jobs2.cab_rides.is_empty() {
        return;
    }
    // C9a: a driver's body booked `cab_no_show_ticks` ago and not yet at the
    // rider's door (his plan held him) is a no-show: the off-screen cab.
    let no_show = Tick::from(world.config.service.cab_no_show_ticks);
    let late = |r: &CabRide| no_show > 0 && r.body && r.stage == 0 && world.tick > r.tick + no_show;
    let keep: Vec<bool> = world.jobs2.cab_rides.iter().map(|r| can_drive(world, r.driver) && !late(r)).collect();
    let gone: Vec<CabRide> = world.jobs2.cab_rides.iter().zip(&keep).filter(|(_, &k)| !k).map(|(r, _)| *r).collect();
    let mut k = keep.into_iter();
    world.jobs2.cab_rides.retain(|_| k.next().unwrap_or(true));
    for r in gone {
        release(world, &r, false);
    }
}

/// The pre-view pass phase 5 (the `fares` shape, OPEN-115): `[legs]` on.
pub fn fares_on(world: &World) -> bool {
    world.config.legs.is_some()
}

/// The `fares` shape: a body ride's rider steps off. `arrived`: at the
/// destination with the driver (the rider's leg ends there); else the
/// driver could not finish (his shift over, jailed, gone off screen, the
/// day's end, round 6 V4: his plan off the fare): the Garage's off-screen
/// cab takes the rider on from where he stands, timed at the cab's speed.
/// Round 6 V4: the fare is paid to the driver at the pick-up (`fare_plan`);
/// one still owed here (never picked up) goes to the off-screen cab. No-op
/// for a ride without a body rider.
fn release(world: &mut World, r: &CabRide, arrived: bool) {
    if !r.body || world.jobs2.passengers.remove(&r.rider).is_none() {
        return;
    }
    let target = match world.comp::<Brain>(r.rider).map(|b| &b.exec) {
        Some(crate::exec::ExecState::GotoTimed { target, arrive_tick, .. }) if *arrive_tick == Tick::MAX => {
            target.clone()
        }
        _ => return,
    };
    let garage = world.comp::<Job>(r.driver).and_then(|j| j.employer);
    if r.fare > 0 {
        if let Some(g) = garage {
            let ride = Ride { from: r.from, to: r.to, fare: r.fare, by: RideBy::Cab(g) };
            // Round 6 V4: a fare still owed here is a ride never picked up
            // (paid at the pick-up, `fare_plan`): the off-screen cab's.
            pay_ride(world, r.rider, &ride, None);
        }
    }
    let arrive = if arrived {
        ownership::jobs2_row(world).fares_driven += 1;
        world.tick
    } else {
        ownership::jobs2_row(world).fares_unmanned += 1;
        let here = crate::exec::walk_origin(world, r.rider).unwrap_or(r.from);
        world.tick + cab_ticks(world, here, target.tile)
    };
    let to = target.tile;
    world.jobs2.riding.insert(r.rider, arrive);
    if let Some(b) = world.comp_mut::<Brain>(r.rider) {
        b.exec = crate::exec::ExecState::GotoTimed {
            target,
            arrive_tick: arrive,
            blocked_since: None,
            walk: None,
            climb: None,
        };
    }
    // The island pass phase 5 (I3): the off-screen cab takes him on in the
    // vehicle he is aboard (else one comes to his door), from where it is.
    if !arrived && crate::systems::motion::on(world) {
        use crate::systems::motion::{self, RideKind};
        let (car, now, reuse) = (crate::components::AssetKind::Car, world.tick, motion::vehicle_of(world, r.rider));
        motion::board_timed(world, r.rider, RideKind::Cab, car, None, reuse, to, now, arrive, false);
    }
}

/// The island pass phase 5 (I3): the cab a body driver's next fare leg
/// drives: the one he is at the wheel of, else the one his fare is aboard.
pub fn fare_vehicle(world: &World, driver: EntityId) -> Option<EntityId> {
    if let Some(v) = crate::systems::motion::vehicle_of(world, driver) {
        return Some(v);
    }
    let r = world.jobs2.cab_rides.iter().find(|r| r.driver == driver && r.body)?;
    crate::systems::motion::vehicle_of(world, r.rider)
}

/// The `fares` shape: the driver `driver` moved; a rider aboard his ride
/// (stage 1, picked up) takes his tile, out on the street (`exec::run`
/// after the driver's step, as an escort's suspect follows the guard).
pub fn carry(world: &mut World, driver: EntityId) {
    // Round 6 V4 (OPEN-240): a driver whose plan is no longer the fare's
    // (another goal pre-empted the Work plan) leaves his rider to the
    // off-screen cab from where the rider stands, paid then (`release`):
    // the rider rode along wherever the driver went instead
    // (`exec_73953 d24 01:16`: a paid cab to a D4 kitchen, 167 min later on
    // a street in D5). Only a rider aboard (stage 1): a driver booked while
    // finishing another plan comes for the waiting rider after it.
    let off_fare = world.comp::<Brain>(driver).and_then(|b| b.plan_goal()).is_some_and(|g| g != GoalKind::Work);
    if off_fare {
        if let Some(i) = world.jobs2.cab_rides.iter().position(|r| r.driver == driver && r.body && r.stage >= 1) {
            let r = world.jobs2.cab_rides.remove(i);
            ownership::jobs2_row(world).fares_dropped += 1;
            release(world, &r, false);
        }
        return;
    }
    let Some(r) = world.jobs2.cab_rides.iter().find(|r| r.driver == driver && r.body && r.stage >= 1).copied() else {
        return;
    };
    if world.jobs2.passengers.get(&r.rider) != Some(&driver) {
        return;
    }
    // The island pass phase 5 (I3): the fare climbs into the cab his driver
    // is at the wheel of; the vehicle carries him (`motion::advance`).
    if let Some(v) = crate::systems::motion::vehicle_of(world, driver) {
        crate::systems::motion::join(world, r.rider, v);
        return;
    }
    let Some(tile) = world.comp::<Position>(driver).filter(|p| p.building.is_none()).map(|p| p.tile) else { return };
    if world.comp::<Position>(r.rider).is_some_and(|p| p.building.is_some()) {
        world.leave_building(r.rider);
    }
    if let Some(p) = world.comp_mut::<Position>(r.rider) {
        p.tile = tile;
    }
}

/// The `fares` shape: `rider`'s plan was dropped while he waited or rode:
/// he leaves the cab where he stands, and the driver's ride is over (no fare).
pub fn passenger_off(world: &mut World, rider: EntityId) {
    world.jobs2.riding.remove(&rider);
    if world.jobs2.passengers.remove(&rider).is_none() {
        return;
    }
    world.jobs2.cab_rides.retain(|r| !(r.body && r.rider == rider));
}

/// Round 6 V4 (OPEN-240): `id` rides a cab under way (a timed leg not yet
/// landed, or aboard a driver's body's cab): `fixes::committed` holds it.
pub fn riding(world: &World, id: EntityId) -> bool {
    // The cab's own leg: its landing tick on the timed GoTo (`Tick::MAX`
    // while a passenger), so an entry a plan reset left behind holds nothing.
    let Some(&t) = world.jobs2.riding.get(&id) else { return false };
    t > world.tick
        && world.comp::<Brain>(id).is_some_and(
            |b| matches!(b.exec, crate::exec::ExecState::GotoTimed { arrive_tick, .. } if arrive_tick == t),
        )
        // A body ride holds once he is aboard (stage 1): waiting at his door
        // for a driver who is late, his next think may still give it up.
        && (t != Tick::MAX
            || (world.jobs2.passengers.contains_key(&id)
                && world.jobs2.cab_rides.iter().any(|r| r.rider == id && r.body && r.stage >= 1)))
}

/// The `fares` shape: the day's end (`daily`): every rider still waiting or
/// aboard goes on by the off-screen cab before the day's rides are cleared.
pub(super) fn release_all(world: &mut World) {
    if world.jobs2.passengers.is_empty() {
        return;
    }
    let rides: Vec<CabRide> = world.jobs2.cab_rides.iter().filter(|r| r.body).copied().collect();
    for r in rides {
        release(world, &r, false);
    }
    world.jobs2.passengers.clear();
}

/// `leisure::spot_of` and the stand-by Rest (`exec::actions`): `id` is on a
/// fare in his Work plan (a ride of his pending and the plan the fare's;
/// another goal's plan that pre-empted it reads its own spot).
pub fn driving(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Work) && on_fare(world, id)
}

/// Lives at work phase 4: is `id` a cab driver with a ride pending (the
/// shadow tool's and `leisure::spot_of`'s question)?
pub fn on_fare(world: &World, id: EntityId) -> bool {
    crate::systems::service::street_on(world) && ride_of(world, id).is_some()
}

/// The tile of the driver's current leg: the rider's door, then the work door.
pub fn fare_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let r = world.jobs2.cab_rides.get(ride_of(world, id)?)?;
    Some(if r.stage == 0 { r.from } else { r.to })
}

/// `exec::start_step`: a driver's leg is a drive, timed at the cab's speed.
pub fn fare_leg(world: &World, id: EntityId, target: &crate::exec::GotoTarget) -> Option<crate::exec::ExecState> {
    // Round 3 S5: the drive to the rank is the cab's too.
    let to_rank = target.building.is_none() && rank_spot(world, id) == Some(target.tile);
    if !on_fare(world, id) && !cab_home(world, id, target) && !to_rank {
        return None;
    }
    let here = crate::exec::walk_origin(world, id)?;
    let arrive_tick = world.tick + cab_ticks(world, here, target.tile);
    Some(crate::exec::ExecState::GotoTimed {
        target: target.clone(),
        arrive_tick,
        blocked_since: None,
        walk: None,
        climb: None,
    })
}

/// The pressure pass R6a (OPEN-217): a cab driver on shift heading back to
/// his Garage from the street drives it (the cab is his): after a drop-off
/// he walked back (`cabbie_10678 d15`: 224 min for 227 tiles, a fare's
/// worth of the shift), standing by far from the riders.
fn cab_home(world: &World, id: EntityId, target: &crate::exec::GotoTarget) -> bool {
    let Some(job) = world.comp::<Job>(id) else { return false };
    target.building.is_some()
        && target.building == job.employer
        && fare_duty(world, job)
        && job.on_shift(world.tick_of_day())
        && crate::systems::service::street_on(world)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
}

/// `plan::plan_for`'s Work arm (after the round): a cab driver on shift
/// with a ride pending drives it, one leg a plan: `GoTo(Spot)` to the
/// rider's door (stage 0), then to the rider's work door (stage 1); the
/// legs read the driver's position as `round_plan` reads a stop. With no
/// ride he stands by at the Garage (`GoTo(Workplace) → Rest`, the Rest
/// ending when a ride is his: `exec::actions`), and the shift's last
/// `ROUND_CUTOFF` is the post's (the work step and the wage, as the
/// round's). No work `ActionKind` is cut short (§ 1.3). The M13 trip roll
/// (crashes) is skipped: the Garage posts no vehicle asset for the cab
/// (deviation, § 14).
pub fn fare_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    if !crate::systems::service::street_on(world) {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    if !fare_duty(world, job) || !job.on_shift(world.tick_of_day()) {
        return None;
    }
    let garage = job.employer?;
    if let Some(i) = ride_of(world, id) {
        let here = world.comp::<Position>(id).filter(|p| p.building.is_none()).map(|p| p.tile);
        let r = world.jobs2.cab_rides[i];
        let done = if r.stage == 0 && here == Some(r.from) {
            world.jobs2.cab_rides[i].stage = 1;
            // The pre-view pass phase 5 (the `fares` shape): the pick-up,
            // both bodies at the door (P3): the encounter now, and he climbs in.
            if r.body {
                let d = world.district_of(r.from);
                let ride = Ride { from: r.from, to: r.to, fare: r.fare, by: RideBy::Cab(garage) };
                ride_encounter(world, r.rider, Some(id), &ride, d);
                // Round 6 V4 (OPEN-240): the fare paid to him as the rider
                // climbs in, both at the door (at the drop-off the rider was
                // already inside: the settle was apart); the ride then
                // carries no fare (`release` pays none).
                if r.fare > 0 {
                    pay_ride(world, r.rider, &ride, Some(id));
                    world.jobs2.cab_rides[i].fare = 0;
                }
                carry(world, id);
            }
            false
        } else {
            r.stage >= 1 && here == Some(r.to)
        };
        if done && r.body {
            world.jobs2.cab_rides.remove(i);
            release(world, &r, true);
            return fare_plan(world, id);
        }
        if !done {
            let step = ActionInstance { action: ActionKind::GoTo(LocationKey::Spot), target: None, tile: None };
            return Some(Plan {
                goal: GoalKind::Work,
                target: Some(garage),
                steps: vec![step],
                started_tick: world.tick,
            });
        }
        world.jobs2.cab_rides.remove(i);
    }
    if !standing_by(world, id) {
        return None;
    }
    // Round 3 S5 (OPEN-217): no fare, he waits at a rank (`rank_tile`),
    // a Rest on the street there, and cruises to the next when the wait is
    // up (`rank_wait_over`). He stood by 9.8 h resting inside the Garage
    // (`cabbie_82150 d23`), the dispatch measuring every rider from there.
    if let Some(rank) = rank_tile(world, id) {
        let here = world.comp::<Position>(id).map(|p| (p.building, p.tile));
        let action = if here == Some((None, rank)) { ActionKind::Rest } else { ActionKind::GoTo(LocationKey::Spot) };
        let steps = vec![ActionInstance { action, target: None, tile: None }];
        return Some(Plan { goal: GoalKind::Work, target: Some(garage), steps, started_tick: world.tick });
    }
    let steps = vec![
        ActionInstance { action: ActionKind::GoTo(LocationKey::Workplace), target: Some(garage), tile: None },
        ActionInstance { action: ActionKind::Rest, target: None, tile: None },
    ];
    Some(Plan { goal: GoalKind::Work, target: Some(garage), steps, started_tick: world.tick })
}

/// Round 3 S5 (OPEN-217): the rank a cab driver standing by waits at: the
/// door of a building of `[verbs] cab_rank_kinds` within `cab_rank_tiles`
/// of his Garage's door, picked on a hash of the driver and the wait
/// (`cab_rank_ticks`: each wait a fresh rank, the cruise). `None` off, for
/// anyone not on a fare trade, or with no such door.
pub fn rank_tile(world: &World, id: EntityId) -> Option<TilePos> {
    let v = world.config.verbs.as_ref()?;
    if v.cab_rank_tiles == 0 || v.cab_rank_kinds.is_empty() {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    if !fare_duty(world, job) {
        return None;
    }
    let door = world.comp::<Building>(job.employer?)?.door;
    let r = v.cab_rank_tiles;
    let mut doors: Vec<TilePos> = Vec::new();
    for &k in &v.cab_rank_kinds {
        for b in world.kind_grid(k).near(door, r) {
            if let Some(bd) = world.comp::<Building>(b).filter(|bd| !bd.demolished && bd.door.manhattan(door) <= r) {
                doors.push(bd.door);
            }
        }
    }
    doors.sort_unstable_by_key(|t| (t.x, t.y));
    doors.dedup();
    if doors.is_empty() {
        return None;
    }
    let slot = world.tick / v.cab_rank_ticks.max(1);
    let u = hash(world, PURPOSE_RANK, u64::from(id.index), slot);
    Some(doors[((u * doors.len() as f32) as usize).min(doors.len() - 1)])
}

/// Round 3 S5: a cab driver resting at his rank (on the street, standing
/// by) since `started` whose wait is up (`cab_rank_ticks`): he cruises on
/// (`exec::actions::finishes_early`). False off.
pub fn rank_wait_over(world: &World, id: EntityId, started: Tick) -> bool {
    let Some(t) = world.config.verbs.as_ref().map(|v| v.cab_rank_ticks.max(1)) else { return false };
    world.comp::<Position>(id).is_some_and(|p| p.building.is_none())
        && started / t != world.tick / t
        && standing_by(world, id)
        && rank_tile(world, id).is_some()
}

/// Round 3 S5: is `id` a cab driver standing by at his rank's tile (the
/// street Rest's start check, `exec::start_step`)?
pub fn at_rank(world: &World, id: EntityId) -> bool {
    standing_by(world, id)
        && world.comp::<Position>(id).is_some_and(|p| p.building.is_none() && rank_tile(world, id) == Some(p.tile))
}

/// Round 3 S5: the rank's tile as the Work plan's `GoTo(Spot)` reads it
/// (standing by, in the Work plan). `leisure::spot_of`.
pub fn rank_spot(world: &World, id: EntityId) -> Option<TilePos> {
    if world.comp::<Brain>(id).and_then(|b| b.plan_goal()) != Some(GoalKind::Work) || !standing_by(world, id) {
        return None;
    }
    rank_tile(world, id)
}

/// A `fare` duty trade's job (the cab driver).
/// The island pass phase 8 (`[island_trades] cab_quota`; OPEN-168): each
/// Garage's first `cab_quota` cab drivers by id who are on shift or within
/// the hour before it (the commute a body's), free and alive: the LOD's
/// class-2 set (`lod::budget_sets`). At 20k no cab driver ranked into the
/// body budget, so `book_cab` never found a body and the Garage's
/// off-screen cab took every fare. Empty off.
pub fn cab_body_set(world: &World) -> Vec<EntityId> {
    let Some(quota) = world.config.island_trades.as_ref().map(|c| usize::from(c.cab_quota)) else {
        return Vec::new();
    };
    if quota == 0 || !crate::systems::service::street_on(world) {
        return Vec::new();
    }
    let tod = world.tick_of_day();
    let lead = ((u64::from(tod) + TICKS_PER_HOUR) % TICKS_PER_DAY) as u16;
    let mut out = Vec::new();
    for &g in world.buildings_of_kind(BuildingKind::Garage) {
        let mut staff: Vec<EntityId> = world
            .staff_of(g)
            .iter()
            .copied()
            .filter(|&s| {
                world.comp::<Job>(s).is_some_and(|j| fare_duty(world, j) && (j.on_shift(tod) || j.on_shift(lead)))
                    && !world.has::<Sentence>(s)
                    && crate::systems::law::living(world, s)
            })
            .collect();
        staff.sort_unstable();
        out.extend(staff.into_iter().take(quota));
    }
    out
}

fn fare_duty(world: &World, job: &Job) -> bool {
    crate::systems::trades::row_of(world, job.role).is_some_and(|r| r.duty == TradeDuty::Fare)
}

/// Lives at work phase 4 (§ 2.3): is `id` a cab driver standing by for a
/// ride: on his shift's workday, before its last `ROUND_CUTOFF`, the shift
/// not closed, no ride pending (`exec::actions`' Rest runs on while true;
/// `drivers_free` reads it)? False off.
pub fn standing_by(world: &World, id: EntityId) -> bool {
    if !crate::systems::service::street_on(world) {
        return false;
    }
    let Some(job) = world.comp::<Job>(id) else { return false };
    if !fare_duty(world, job) || !job.on_shift(world.tick_of_day()) {
        return false;
    }
    let key = job.shift_key_at(world.tick);
    job.last_shift_day != Some(key)
        && crate::exec::routine::workday_of(world, id, job, key)
        && job
            .shift_end(world.tick)
            .is_some_and(|e| e.saturating_sub(world.tick) > crate::systems::trades::ROUND_CUTOFF)
        && ride_of(world, id).is_none()
}
