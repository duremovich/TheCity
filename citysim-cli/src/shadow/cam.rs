use super::*;

// ---------------------------------------------------------------------------
// Shadow V6: the camera lens and V5 bug 20's labels
// ---------------------------------------------------------------------------

/// The camera's place of `id` this minute (an index into `CAM_PLACES`):
/// held (a sentence), Statistical, Coarse, a vehicle (the El or a flyer,
/// boarding included, or a walk with a vehicle under the agent), then by
/// the building: Home, work (the employer, an exec's Hq, or a work-class or
/// Meeting step anywhere), another building, else the street.
pub(super) fn cam_place(world: &World, id: EntityId, act: &str) -> usize {
    if world.has::<Sentence>(id) {
        return CAM_HELD;
    }
    if let Some(b) = world.comp::<Brain>(id) {
        match b.lod {
            Lod::Statistical => return CAM_STAT,
            Lod::Coarse => return CAM_COARSE,
            Lod::Full => {}
        }
        let driving = world.comp::<Kit>(id).is_some_and(|k| k.driving.is_some());
        match b.exec {
            ExecState::Fly { .. } | ExecState::Ride { .. } => return CAM_VEHICLE,
            ExecState::Goto { .. } if driving => return CAM_VEHICLE,
            _ => {}
        }
        if pass_vehicle(world, id).is_some() || citysim::systems::motion::vehicle_of(world, id).is_some() {
            return CAM_VEHICLE;
        }
    }
    let Some(bl) = world.comp::<Position>(id).and_then(|p| p.building) else { return CAM_STREET };
    if world.comp::<Household>(id).and_then(|h| h.home) == Some(bl) {
        return CAM_HOME;
    }
    let employer = world.comp::<Job>(id).and_then(|j| j.employer).or_else(|| {
        citysim::systems::life::exec_corp(world, id).and_then(|c| citysim::systems::life::hq_of(world, c, id))
    });
    if employer == Some(bl) || act == "work" || act == "meeting" {
        CAM_WORK
    } else {
        CAM_BUILDING
    }
}

/// The city milestone C7: the diary's line for the climb `id` is on (a Full
/// body's `ExecState::Climb`, a Coarse timed leg's climbs) and its key (the
/// climb's start), `None` off one.
pub(super) fn climb_line(world: &World, id: EntityId) -> Option<(u64, String)> {
    use citysim::exec::ClimbLeg;
    let b = world.comp::<Brain>(id)?;
    let way = |lift: bool| if lift { "the lift" } else { "the stairs" };
    let leg = |c: &ClimbLeg| {
        let dir = if c.to > c.from { "up" } else { "down" };
        let wait = if c.plan.lift { format!(", {} min for the car", c.plan.wait) } else { String::new() };
        format!(
            "{dir} by {} in {}: floor {} to {}{wait}, {} min",
            way(c.plan.lift),
            place_of_building(world, c.core),
            c.from,
            c.to,
            c.plan.ticks
        )
    };
    match &b.exec {
        ExecState::Climb { core, from, to, depart, board, arrive_tick, lift, .. } => {
            let dir = if to > from { "up" } else { "down" };
            let q = citysim::systems::levels::queue(world, *core, *depart);
            let wait = if *lift {
                format!(
                    ", {} min waiting in the lobby ({q:.0} called the car, this call too)",
                    board.saturating_sub(*depart)
                )
            } else {
                String::new()
            };
            Some((
                *depart * 2,
                format!(
                    "CLIMB {dir} by {} in {}: floor {from} to {to}{wait}, {} min in all",
                    way(*lift),
                    place_of_building(world, *core),
                    arrive_tick.saturating_sub(*depart)
                ),
            ))
        }
        ExecState::GotoTimed { climb: Some(c), .. } => {
            let parts: Vec<String> = c.down.iter().chain(c.up.iter()).map(leg).collect();
            Some((c.depart * 2 + 1, format!("CLIMB (timed leg) {}", parts.join("; then "))))
        }
        _ => None,
    }
}

/// The exec state as a small code (part of an unchanged stretch's key).
fn exec_code(e: &ExecState) -> u8 {
    match e {
        ExecState::Idle => 0,
        ExecState::Goto { .. } => 1,
        ExecState::GotoTimed { .. } => 2,
        ExecState::Use { .. } => 3,
        ExecState::Wait { .. } => 4,
        ExecState::Fly { .. } => 5,
        ExecState::Ride { .. } => 6,
        ExecState::JackedIn { .. } => 7,
        ExecState::Climb { .. } => 8,
    }
}

fn exec_name(e: &ExecState) -> &'static str {
    match e {
        ExecState::Idle => "Idle",
        ExecState::Goto { .. } => "Goto",
        ExecState::GotoTimed { .. } => "GotoTimed (Coarse walk)",
        ExecState::Use { .. } => "Use",
        ExecState::Wait { .. } => "Wait",
        ExecState::Fly { .. } => "Fly",
        ExecState::Ride { .. } => "Ride (the El)",
        ExecState::JackedIn { .. } => "JackedIn",
        ExecState::Climb { .. } => "Climb (the stairs or the lift)",
    }
}

/// Chebyshev tiles a tick in this exec state may cover: a walk the pace's
/// tiles a tick (round 9 Y4: `[exec] walk_tiles`, a walker's step is
/// `step_units` or fewer), a drive `[vehicles]
/// max_steps_per_tick`, a flyer `1 / flyer_ticks_per_tile`, the El `1 /
/// ride_ticks_per_tile` (the track is no shorter than the Chebyshev
/// distance), a Coarse timed walk the same (generous: it is timed at the
/// pace a Manhattan tile); anything else 0.
fn exec_speed(world: &World, id: EntityId, e: &ExecState) -> f32 {
    if let Some(v) = pass_vehicle(world, id) {
        return v;
    }
    match e {
        ExecState::Goto { .. } if world.comp::<Kit>(id).is_some_and(|k| k.driving.is_some()) => {
            f32::from(world.config.vehicles.max_steps_per_tick.max(1))
        }
        // Round 9 Y4: chromed legs (`Kit.walk_mult` under 1) step faster.
        ExecState::Goto { .. } | ExecState::GotoTimed { .. } => {
            let m = world.comp::<Kit>(id).map_or(1.0, |k| k.walk_mult).clamp(0.05, 1.0);
            (world.config.exec.walk_tiles(1).max(1) as f32 / m).ceil()
        }
        ExecState::Fly { .. } => 1.0 / world.config.vehicles.flyer_ticks_per_tile.max(0.01),
        ExecState::Ride { .. } => 1.0 / world.config.transit.ride_ticks_per_tile.max(0.01),
        // Round 9 Y4: an Idle body whose step is a walk (a segment's end
        // re-starting it, a plan's first step) walks the next minute at the
        // pace (it stepped one tile before, inside the slack).
        ExecState::Idle
            if world.comp::<Brain>(id).and_then(|b| b.current_step()).is_some_and(|s| {
                matches!(s.action, ActionKind::GoTo(_) | ActionKind::FleeToHome | ActionKind::Escort)
            }) =>
        {
            let m = world.comp::<Kit>(id).map_or(1.0, |k| k.walk_mult).clamp(0.05, 1.0);
            (world.config.exec.walk_tiles(1).max(1) as f32 / m).ceil()
        }
        _ => 0.0,
    }
}

/// Shadow V7 (tool): the pre-view pass's timed vehicle legs of a Full body,
/// which V6's lens read as a street walk at 1 tile a tick: the escort van
/// (an `Escort` step's `GotoTimed`, `exec::escort_leg`; its suspect, cuffed,
/// rides along), a cab driver's fare leg (`GotoTimed` on a fare,
/// `exec::fare_segment`) and a rider aboard a body cab (`Jobs2State.
/// passengers`, the ride under way). Returns the leg's tiles a tick, the
/// cab's timed speed (`streetwork::cab_ticks`: the walk's pace over the
/// Car's `timed_mult` a Manhattan tile). `None`: not one of these.
fn pass_vehicle(world: &World, id: EntityId) -> Option<f32> {
    let b = world.comp::<Brain>(id)?;
    let timed = |x: EntityId| {
        world.comp::<Brain>(x).is_some_and(|bx| matches!(bx.exec, ExecState::GotoTimed { .. }) && bx.lod == Lod::Full)
    };
    let van = |x: EntityId| {
        timed(x)
            && world.comp::<Brain>(x).and_then(|bx| bx.current_step()).is_some_and(|s| s.action == ActionKind::Escort)
    };
    let fare = timed(id) && citysim::systems::streetwork::on_fare(world, id);
    let aboard = world.jobs2.passengers.get(&id).is_some_and(|&d| {
        world.jobs2.cab_rides.iter().any(|r| r.body && r.rider == id && r.driver == d && r.stage >= 1)
    });
    let cuffed = b.cuffed_by.is_some_and(van);
    if !(van(id) || fare || aboard || cuffed) {
        return None;
    }
    // The pressure pass R2: the vans drive the road at its own speed
    // (`vehicles::road_ticks`), faster than the cab's Coarse blend.
    let car = citysim::components::AssetKind::Car;
    let m = citysim::systems::vehicles::timed_mult_of(world, car).min(world.config.vehicles.mult_of(car, 0));
    let e = &world.config.exec;
    Some(e.walk_tiles_per_tick.max(1) as f32 / (e.move_ticks_full.max(1) as f32 * m).max(0.01))
}

/// Shadow V8: a vehicle in motion's own move over the last two ticks (its
/// route's place then and now; a driven car `[vehicles] max_steps_per_tick`
/// a tick). Generous by a tick, so a boarding minute is never a jump for
/// the vehicle's pace alone.
fn veh_step(world: &World, v: EntityId) -> f32 {
    use citysim::systems::{motion, transit};
    let Some(m) = world.motions.get(&v) else { return 0.0 };
    let t = world.tick;
    let at = |k: Tick| match &m.route {
        motion::Route::Timed { way, depart, arrive } => Some(motion::timed_pos(way, *depart, *arrive, k)),
        motion::Route::Line { line, train } => {
            transit::train_at(world, *line, u64::from(*train), k).and_then(|a| transit::train_tile(world, *line, a))
        }
        motion::Route::Driven => None,
    };
    match (at(t.saturating_sub(2)), at(t)) {
        (Some(a), Some(b)) => chebyshev(a, b) as f32,
        _ => 2.0 * f32::from(world.config.vehicles.max_steps_per_tick.max(1)),
    }
}

fn chebyshev(a: citysim::TilePos, b: citysim::TilePos) -> u32 {
    u32::from(a.x.abs_diff(b.x)).max(u32::from(a.y.abs_diff(b.y)))
}

/// Two agents in one building, or both outdoors within `COLOC_TILES`.
fn colocated(world: &World, a: EntityId, b: EntityId) -> bool {
    let (Some(x), Some(y)) = (world.comp::<Position>(a), world.comp::<Position>(b)) else { return false };
    match (x.building, y.building) {
        (Some(p), Some(q)) => p == q,
        (None, None) => x.tile.manhattan(y.tile) <= COLOC_TILES,
        _ => false,
    }
}

/// The camera's minute: the place, the jump check, the unchanged stretch.
/// Reads only; no RNG.
pub(super) fn cam_minute(world: &World, t: &mut Track, now: Tick, act: &'static str, here: &str) {
    let id = t.id;
    let place = cam_place(world, id, act);
    t.cam.days.entry(now / TICKS_PER_DAY).or_default().place_min[place] += 1;
    let Some(pos) = world.comp::<Position>(id).cloned() else { return };
    let brain = world.comp::<Brain>(id);
    // Shadow V7: the work-class run on one plan step, and the work tiles.
    let step_key = brain.and_then(|b| b.plan.as_ref().map(|p| (p.started_tick, b.plan_step)));
    match step_key.filter(|_| act == "work") {
        Some(k) => {
            let start = match t.cam.work_run {
                Some((rk, s)) if rk == k => s,
                _ => now,
            };
            t.cam.work_run = Some((k, start));
            let len = u32::try_from(now - start + 1).unwrap_or(u32::MAX);
            let d = t.cam.days.entry(start / TICKS_PER_DAY).or_default();
            d.work_run_max = d.work_run_max.max(len);
            t.cam.days.entry(now / TICKS_PER_DAY).or_default().work_tiles.insert(pos.tile);
        }
        None => t.cam.work_run = None,
    }
    let lod = brain.map(|b| b.lod);
    let held = world.has::<Sentence>(id);
    // The jump: the allowance accrued in the state the tick ran under. Inside
    // a building the body is at its door (the camera draws the building; a
    // step in through the door lands on an interior tile).
    let at = pos.building.and_then(|b| world.comp::<Building>(b)).map_or(pos.tile, |b| b.door);
    t.cam.allow += t.cam.speed;
    // Shadow V8: a rider aboard a vehicle in motion (a car, cab, flyer, van
    // or El train, `systems/motion.rs`) is where the vehicle is, so the
    // vehicle's own move since the last minute is allowance. A snap onto
    // the vehicle at boarding still reads as a jump; the vehicle's own
    // continuity is the `run --legs` `#ride` device's to judge.
    // `Motion::pos` is where `motion::advance` set its riders (the route's
    // place at the tick's own time; `motion::pos_of` reads the timetable
    // at `world.tick` and runs a tick ahead of the riders).
    let veh = citysim::systems::motion::vehicle_of(world, id).and_then(|v| world.motions.get(&v).map(|m| (v, m.pos)));
    match (veh, t.cam.veh) {
        (Some((v, p)), Some((pv, pp))) if v == pv => t.cam.allow += chebyshev(pp, p) as f32,
        // The boarding minute: the vehicle may already have pulled away
        // (a train departs the tick its rider boards), so its own step is
        // allowance too.
        (Some((v, _)), _) => t.cam.allow += veh_step(world, v),
        _ => {}
    }
    t.cam.veh = veh;
    let mut jump_line: Option<String> = None;
    match t.cam.anchor.clone() {
        None => t.cam.anchor = Some((at, now, here.to_string())),
        Some((tile, since, from)) if tile != at => {
            let dist = chebyshev(tile, at);
            if dist as f32 > t.cam.allow + CAM_JUMP_SLACK {
                let body = t.cam.prev_lod == Some(Lod::Full) && lod == Some(Lod::Full) && !t.cam.prev_held && !held;
                let d = t.cam.days.entry(now / TICKS_PER_DAY).or_default();
                d.jumps += 1;
                d.jumps_body += u32::from(body);
                d.max_jump = d.max_jump.max(dist);
                d.first_jump.get_or_insert(now);
                let lods = match (t.cam.prev_lod, lod) {
                    (Some(a), Some(b)) if a != b => format!("LOD {a:?} -> {b:?}"),
                    (_, Some(b)) => format!("LOD {b:?}"),
                    _ => "no LOD".into(),
                };
                let jail = match (t.cam.prev_held, held) {
                    (false, true) => ", sentenced",
                    (true, false) => ", released",
                    (true, true) => ", held",
                    _ => "",
                };
                let step = brain.and_then(|b| b.current_step()).map_or("no step".into(), |s| format!("{:?}", s.action));
                jump_line = Some(format!(
                    "JUMP{} {dist} tiles in {} min (allowance {:.0}): {from} -> {here}; {lods}{jail}; exec {}, {step}",
                    if body { " (a Full body)" } else { "" },
                    now.saturating_sub(since),
                    t.cam.allow + CAM_JUMP_SLACK,
                    brain.map_or("none", |b| exec_name(&b.exec)),
                ));
            }
            t.cam.anchor = Some((at, now, here.to_string()));
            t.cam.allow = 0.0;
        }
        Some(_) => {}
    }
    t.cam.speed = brain.map_or(0.0, |b| exec_speed(world, id, &b.exec));
    t.cam.prev_lod = lod;
    t.cam.prev_held = held;
    if let Some(l) = jump_line {
        t.log(now, "camera", l);
    }
    // The unchanged stretch.
    let key: StillKey = (
        pos.tile,
        pos.building,
        brain.and_then(|b| b.current_step().map(|s| s.action)),
        brain.map_or(u8::MAX, |b| exec_code(&b.exec)),
    );
    if t.cam.still_key != Some(key) {
        close_still(t, now);
        t.cam.still_key = Some(key);
        t.cam.still_start = now;
        t.cam.still_act = act;
        t.cam.still_place = here.to_string();
    }
}

/// Close the unchanged stretch under way at `now` (its minutes credit the
/// day it began).
pub(super) fn close_still(t: &mut Track, now: Tick) {
    if t.cam.still_key.take().is_none() {
        return;
    }
    let len = u32::try_from(now.saturating_sub(t.cam.still_start)).unwrap_or(u32::MAX);
    let rec = (len, t.cam.still_start, t.cam.still_act, t.cam.still_place.clone());
    let awake = t.cam.still_act != "sleep";
    let d = t.cam.days.entry(t.cam.still_start / TICKS_PER_DAY).or_default();
    if d.still.as_ref().is_none_or(|s| len > s.0) {
        d.still = Some(rec.clone());
    }
    if awake && d.still_awake.as_ref().is_none_or(|s| len > s.0) {
        d.still_awake = Some(rec);
    }
}

/// A settlement naming a person (a flow note or a `Served` memory): counted,
/// and logged when the counterpart is not a body beside the pick, or the
/// pick is asleep (V5 bugs 6 and 7: the encounter narrates a settlement the
/// body was not present for).
pub(super) fn cam_settle(world: &World, t: &mut Track, now: Tick, other: EntityId, what: &str) {
    cam_settle_with(world, t, now, other, what, true);
}

/// `cam_settle`, the `[off]` narration left to the caller when `narrate_off`
/// is false (round 4 T7: an encounter's memory line says "off screen"
/// itself; a SETTLED APART line is always logged). True for an `[off]` one.
pub(super) fn cam_settle_with(
    world: &World,
    t: &mut Track,
    now: Tick,
    other: EntityId,
    what: &str,
    narrate_off: bool,
) -> bool {
    if other == t.id || other == EntityId::NONE || !world.has::<Identity>(other) {
        return false;
    }
    let asleep = activity(world, t.id) == "sleep";
    let stat = world.comp::<Brain>(other).is_some_and(|b| b.lod == Lod::Statistical);
    let apart = !stat && !colocated(world, t.id, other);
    let d = t.cam.days.entry(now / TICKS_PER_DAY).or_default();
    d.settles += 1;
    d.settles_apart += u32::from(apart);
    d.settles_stat += u32::from(stat);
    d.settles_asleep += u32::from(asleep);
    if apart || stat || asleep {
        let where_ = if stat {
            "Statistical (off screen)".to_string()
        } else if apart {
            format!("at {}", place(world, other))
        } else {
            "beside me".to_string()
        };
        let me = if asleep { format!("; I am asleep at {}", place(world, t.id)) } else { String::new() };
        // The pre-view pass phase 4 (P3): an encounter with an off-screen
        // party, the pick awake, is narrated `[off]`; a body elsewhere or the
        // pick asleep is the bug (SETTLED APART).
        let off = stat && !asleep;
        let tag = if off { "[off]" } else { "SETTLED APART:" };
        if narrate_off || !off {
            t.log(now, "camera", format!("{tag} {what}; {} {where_}{me}", nm(world, other)));
        }
        return off;
    }
    false
}

/// A `PlanAborted` text's reason: the `FailReason` of a failed step, else
/// how the plan ended (an interrupt, unplannable, a LOD demotion).
pub(super) fn abort_reason(text: &str) -> String {
    if let Some(i) = text.find(" failed at ") {
        if let Some(j) = text[i..].find(": ") {
            let r: String = text[i + j + 2..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if !r.is_empty() {
                return r;
            }
        }
        return "Failed".into();
    }
    if text.contains("unplannable") {
        "Unplannable".into()
    } else if text.contains("LodDemotion") {
        "LodDemotion".into()
    } else if text.contains(" -> ") {
        "Interrupted".into()
    } else {
        "Other".into()
    }
}

/// A flow's name in the diary: the sim's `Flow`, but the school fee (a
/// `Flow::Contract` of `[ages] school_fee` from a person to a School's
/// owner or the city) is `SchoolFee` (V5 bug 20).
pub(super) fn flow_label(
    world: &World,
    flow: citysim::systems::ownership::Flow,
    from: Option<EntityId>,
    to: Option<EntityId>,
    coins: i64,
) -> String {
    use citysim::systems::ownership::Flow as F;
    let school_owner =
        |o: Option<EntityId>| world.buildings_of_kind(BuildingKind::School).iter().any(|&b| world.owner_of(b) == o);
    if flow == F::Contract
        && coins == world.config.ages.school_fee
        && from.is_some_and(|f| world.has::<Identity>(f))
        && school_owner(to)
    {
        return "SchoolFee".into();
    }
    format!("{flow:?}")
}

/// The `[[serves]]` row `id` serves by, as `service::row_of_agent` (private
/// there) reads it: the Job's row, else a postless pitch owner's trade row;
/// a gang post (the pimp, the dealer) by its key when neither.
fn serve_row(world: &World, id: EntityId) -> Option<&citysim::config::ServesCfg> {
    let by_key = |k: &str| world.config.serves.iter().find(|r| r.key == k);
    let row = match world.comp::<Job>(id) {
        Some(j) => citysim::systems::service::row_for(world, j.role),
        None => world.jobs2.pitches.get(&id).and_then(|p| world.config.trade(p.trade)).and_then(|t| by_key(&t.id)),
    };
    row.or_else(|| {
        if citysim::systems::life::exec_corp(world, id).is_some() {
            by_key("exec")
        } else if world.jobs2.vice.pimps.values().any(|&p| p == id) {
            by_key("pimp")
        } else if world.dealers.values().flatten().any(|&d| d == id) {
            by_key("dealer")
        } else {
            None
        }
    })
}

/// V5 bug 20: a `Served` memory as its kind and side. The memory carries
/// neither, so the server is read from the regulars ledger (the encounter
/// stamps the server's row for the served today), the pimp from his
/// district's street pitch, else from the pair's rows (the door, the
/// Camp's or Jail's count, a corp's table: the encounters with no ledger;
/// a super's stop, whose Block can overflow the ledger's cap in one tick).
/// The Camp's and the Jail's count is never "a regular".
pub(super) fn served_text(world: &World, holder: EntityId, e: &MemoryEntry) -> String {
    use citysim::components::ServeKind as K;
    let Some(o) = e.subject else { return format!("served someone unnamed (valence {:+.2})", e.valence) };
    let today = u16::try_from(world.day()).unwrap_or(u16::MAX);
    let fresh = |server: EntityId, guest: EntityId| {
        citysim::systems::service::regulars_of(world, server).iter().any(|r| r.who == guest && r.last_day == today)
    };
    let is_pimp = |p: EntityId| world.jobs2.vice.pimps.values().any(|&x| x == p);
    let pitch = |w: EntityId| world.jobs2.pitches.contains_key(&w);
    let is_exec =
        |x: EntityId| world.corps().into_iter().any(|c| world.comp::<Corp>(c).and_then(|k| k.exec) == Some(x));
    let row_kind = |x: EntityId| serve_row(world, x).map(|r| (r.kind, r.key.clone()));
    let unled = |x: EntityId| row_kind(x).is_some_and(|(k, _)| matches!(k, K::Door | K::Inmate | K::Tenant));
    // (the server, the row's kind and key)
    let side: Option<(EntityId, K, String)> = if is_pimp(holder) && pitch(o) {
        Some((holder, K::Staff, "pimp".into()))
    } else if is_pimp(o) && pitch(holder) {
        Some((o, K::Staff, "pimp".into()))
    } else if fresh(holder, o) {
        row_kind(holder).map(|(k, key)| (holder, k, key))
    } else if fresh(o, holder) {
        row_kind(o).map(|(k, key)| (o, k, key))
    } else if is_exec(o) {
        Some((o, K::Attendee, "exec".into()))
    } else if unled(o) {
        row_kind(o).map(|(k, key)| (o, k, key))
    } else if unled(holder) {
        row_kind(holder).map(|(k, key)| (holder, k, key))
    } else if let Some((K::Pupil, key)) = row_kind(o) {
        // The pressure pass S6 (OPEN-224): the teacher met at the door by a
        // parent with a row of his own (no ledger on the parent's side).
        Some((o, K::Pupil, key))
    } else if let Some((K::Pupil, key)) = row_kind(holder) {
        Some((holder, K::Pupil, key))
    } else {
        match (row_kind(holder), row_kind(o)) {
            (Some((k, key)), None) => Some((holder, k, key)),
            (None, Some((k, key))) => Some((o, k, key)),
            _ => None,
        }
    };
    let who = nm(world, o);
    let Some((server, kind, key)) = side else {
        let rk = |x: EntityId| serve_row(world, x).map_or("none".to_string(), |r| r.key.clone());
        return format!(
            "served-pair with {who} (kind and side unknown: my row {}, theirs {}) (valence {:+.2})",
            rk(holder),
            rk(o),
            e.valence
        );
    };
    let mine = server == holder;
    let text = match (kind, mine) {
        (K::Visit, true) => format!("served {who} (a customer, at the {key}'s counter)"),
        (K::Visit, false) => format!("was served by {who} (as a customer; {who} the {key})"),
        (K::Patient, true) => format!("treated {who} (a patient)"),
        (K::Patient, false) => format!("was treated by {who} (as a patient; the {key})"),
        // The pressure pass S6 (OPEN-224): V11's "a pupil's parent" was the
        // pupil (an infant on the roll). A pupil now writes no memory; the
        // parent is named with the child on today's roll here.
        (K::Pupil, true) if world.has::<Child>(o) => format!("taught {who} (a pupil)"),
        (K::Pupil, true) => {
            let school = world.comp::<Job>(holder).and_then(|j| j.employer);
            let kid = citysim::systems::demography::children_of_agent(world, o).into_iter().find(|&c| {
                school.is_some()
                    && citysim::systems::ages::place_of(world, c) == school.map(citysim::systems::ages::Place::School)
            });
            match kid {
                Some(c) => format!("met {who} at the door (the parent of {}, a pupil)", nm(world, c)),
                None => format!("met {who} at the door (a parent; no child of theirs on today's roll)"),
            }
        }
        (K::Pupil, false) => format!("met {who} at the door (my child's teacher)"),
        (K::Inmate, true) if key == "camp_warden" => format!("counted {who} (the Camp's headcount)"),
        (K::Inmate, true) => format!("fed and counted {who} (an inmate)"),
        (K::Inmate, false) if pitch(holder) || world.has::<Sentence>(holder) => {
            format!("was counted by {who} (as an inmate; the {key})")
        }
        (K::Inmate, false) => format!("visited the Camp or the cells, {who} on duty (the {key})"),
        (K::Tenant, true) => format!("called on {who} (a tenant, on the round)"),
        (K::Tenant, false) => format!("was called on by {who} (as a tenant; my super)"),
        (K::Fare, true) => format!("took {who} as a fare (the {key})"),
        (K::Fare, false) => format!("rode with {who} (as a fare; the {key})"),
        (K::Parcel, true) => format!("handed a parcel to {who} (the recipient)"),
        (K::Parcel, false) => format!("took a parcel from {who} (the courier)"),
        (K::Client, true) => format!("served {who} (a client, the {key})"),
        (K::Client, false) => format!("was served by {who} (as a client; the {key})"),
        (K::Door, true) => format!("kept the door on {who} (a patron)"),
        (K::Door, false) => format!("met {who} at the door (the bouncer)"),
        (K::Staff, true) if key == "pimp" => format!("collected from {who} (my street worker)"),
        (K::Staff, false) if key == "pimp" => format!("was collected by {who} (my pimp)"),
        (K::Staff, true) => format!("oversaw {who} (a co-worker, the house's staff)"),
        (K::Staff, false) => format!("was overseen by {who} (a co-worker, the {key})"),
        (K::Buyer, true) => format!("sold to {who} (a buyer, the {key})"),
        (K::Buyer, false) => format!("bought from {who} (as a buyer; the {key})"),
        (K::Attendee, true) if key == "exec" => format!("met {who} at the table (an attendee)"),
        (K::Attendee, false) if key == "exec" => format!("sat at {who}'s table (a Meeting)"),
        (K::Attendee, true) => format!("fought before {who} (a bettor)"),
        (K::Attendee, false) => format!("watched {who} fight (as a bettor)"),
        (K::Source, true) => format!("interviewed {who} (a source)"),
        (K::Source, false) => format!("was interviewed by {who} (as a source; the reporter)"),
        (K::Crate, _) => format!("loaded crates with {who}"),
        (K::Broker, true) => format!("brokered for {who}"),
        (K::Broker, false) => format!("was brokered by {who}"),
    };
    let regular = kind != K::Inmate
        && citysim::systems::service::regulars_of(world, server)
            .iter()
            .any(|r| r.who == if mine { o } else { holder } && r.visits >= 2);
    format!("{text}{} (valence {:+.2})", if regular { ", a regular" } else { "" }, e.valence)
}

/// A courier pick's parcels handed over this tick: those of the yard gone
/// from the pending list while the pick stands in their stop (V5 bug 20).
pub(super) fn is_courier(world: &World, id: EntityId) -> bool {
    world
        .comp::<Job>(id)
        .and_then(|j| citysim::systems::service::row_for(world, j.role))
        .is_some_and(|r| r.kind == citysim::components::ServeKind::Parcel)
}

pub(super) fn cam_parcels(world: &World, t: &mut Track, now: Tick) {
    let id = t.id;
    let job = world.comp::<Job>(id);
    if !is_courier(world, id) {
        t.cam.parcels.clear();
        return;
    }
    let yard = job.and_then(|j| j.employer);
    let cur: Vec<citysim::systems::streetwork::Parcel> =
        world.jobs2.parcels.iter().copied().filter(|p| Some(p.yard) == yard).collect();
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let handed: Vec<citysim::systems::streetwork::Parcel> =
        t.cam.parcels.iter().copied().filter(|p| !cur.contains(p) && here == Some(p.at)).collect();
    for p in handed {
        t.cam.parcels_handed += 1;
        let line = format!(
            "PARCEL handed over at {}: from {} for {} (fee {})",
            place_of_building(world, p.at),
            nm(world, p.from),
            nm(world, p.to),
            money(p.fee)
        );
        t.log(now, "parcel", line);
    }
    t.cam.parcels = cur;
}

pub(super) fn hours(m: u32) -> String {
    format!("{:.1}", f64::from(m) / 60.0)
}

/// The diary's camera block (Shadow V6), after the header.
pub(super) fn camera_block(t: &Track) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "## Camera (Shadow V6)\n");
    let _ = writeln!(
        md,
        "Hours by place; plans started and aborted; jumps (a tile change beyond the walk/ride allowance; body: Full and \
         free at both ends) and the largest; the longest unchanged stretch (tile, building, step, exec) begun that day, \
         all and awake; settlements naming a person (a flow or a Served memory: apart = the counterpart a body \
         elsewhere, stat = Statistical, asleep = the pick asleep).\n"
    );
    let _ = writeln!(
        md,
        "| day | {} | plans | aborts | jumps (body) | max jump | still | awake still | settles (apart/stat/asleep) |",
        CAM_PLACES.join(" | ")
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut notes: Vec<String> = Vec::new();
    for (day, d) in &t.cam.days {
        let _ = write!(md, "| d{day} |");
        for m in d.place_min {
            let _ = write!(md, " {} |", hours(m));
        }
        let aborts: u32 = d.aborts.values().sum();
        let still = d.still.as_ref().map_or("-".into(), |s| format!("{}m {}", s.0, s.2));
        let awake = d.still_awake.as_ref().map_or("-".into(), |s| format!("{}m {}", s.0, s.2));
        let _ = writeln!(
            md,
            " {} | {aborts} | {} ({}) | {} | {still} | {awake} | {} ({}/{}/{}) |",
            d.plans, d.jumps, d.jumps_body, d.max_jump, d.settles, d.settles_apart, d.settles_stat, d.settles_asleep
        );
        let reasons: Vec<String> = d.aborts.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let mut n = format!("d{day}: aborts {}", if reasons.is_empty() { "none".into() } else { reasons.join(", ") });
        if let Some(s) = &d.still_awake {
            let _ = write!(n, "; awake still from {} at {}", clock(s.1), s.3);
        }
        if let Some(j) = d.first_jump {
            let _ = write!(n, "; first jump {}", clock(j));
        }
        // Shadow V7: the shift's shape and the long legs.
        if d.work_run_max > 0 {
            let _ = write!(n, "; longest work action {} min on {} work tile(s)", d.work_run_max, d.work_tiles.len());
        }
        if d.goto_long > 0 {
            let _ = write!(n, "; {} GoTo leg(s) over {CAM_LONG_GOTO_MIN} min", d.goto_long);
        }
        notes.push(n);
    }
    let _ = writeln!(md);
    for n in notes {
        let _ = writeln!(md, "- {n}");
    }
    if t.cam.parcels_handed > 0 {
        let _ = writeln!(md, "- parcels handed over in the window: {}", t.cam.parcels_handed);
    }
    let _ = writeln!(md);
    md
}

/// `camera.tsv`'s header (one row per pick and day).
pub(super) const CAMERA_TSV_HEADER: &str = "arch\tindex\tname\tday\thome_h\twork_h\tbuilding_h\tstreet_h\tvehicle_h\theld_h\tstat_h\t\
coarse_h\tplans\taborts\taborts_by_reason\tjumps\tjumps_body\tmax_jump\tfirst_jump\tstill_min\tstill_start\tstill_class\t\
awake_still_min\tawake_still_start\tawake_still_class\tawake_still_place\tsettles\tsettles_apart\tsettles_stat\t\
settles_asleep\twork_longest_min\twork_tiles\tgoto_over_120";

pub(super) fn camera_rows(t: &Track, out: &mut String) {
    for (day, d) in &t.cam.days {
        let _ = write!(out, "{}\t{}\t{}\t{day}", t.arch, t.id.index, t.name);
        for m in d.place_min {
            let _ = write!(out, "\t{}", hours(m));
        }
        let reasons: Vec<String> = d.aborts.iter().map(|(k, n)| format!("{k}:{n}")).collect();
        let st = |s: &Option<(u32, Tick, &'static str, String)>| {
            s.as_ref().map_or("0\t-\t-".to_string(), |s| format!("{}\t{}\t{}", s.0, clock(s.1), s.2))
        };
        let awake_place = d.still_awake.as_ref().map_or("-".to_string(), |s| s.3.replace('\t', " "));
        let _ = writeln!(
            out,
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{awake_place}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            d.plans,
            d.aborts.values().sum::<u32>(),
            if reasons.is_empty() { "-".into() } else { reasons.join(",") },
            d.jumps,
            d.jumps_body,
            d.max_jump,
            d.first_jump.map_or("-".into(), clock),
            st(&d.still),
            st(&d.still_awake),
            d.settles,
            d.settles_apart,
            d.settles_stat,
            d.settles_asleep,
            d.work_run_max,
            d.work_tiles.len(),
            d.goto_long
        );
    }
}
