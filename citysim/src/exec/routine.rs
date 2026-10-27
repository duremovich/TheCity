//! Shift helpers shared by the Work goal gate and the planner, plus the two
//! plans that bypass GOAP: Idle, and the commute to a shift that has not
//! started yet (the planner cannot express "go early and wait").

use crate::components::{ActionInstance, Building, BuildingKind, GoalKind, Household, Inventory, Job, Position, Role};
use crate::entity::EntityId;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::world::World;

/// Extra ticks of slack when leaving for work.
const DEPARTURE_MARGIN: u64 = 60;
/// An agent already at the workplace waits for a shift this far off.
const WAIT_AT_WORK_MAX: u64 = 600;

fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

fn plan(goal: GoalKind, target: Option<EntityId>, steps: Vec<ActionInstance>, tick: u64) -> Plan {
    Plan { goal, target, steps, started_tick: tick }
}

/// The location key of a role's workplace (the v1 table).
pub fn workplace_key(role: Role) -> LocationKey {
    match role {
        Role::Farmer => LocationKey::Farm,
        Role::Guard => LocationKey::Jail,
        Role::Clerk => LocationKey::Market,
        Role::Bartender => LocationKey::Bar,
        Role::Gravedigger | Role::Sanitation => LocationKey::Cemetery,
        // M13 D16: a Clinic's staff work at their employer (phase 3 gives
        // the Clinic its key); D47: a Garage's at the Garage, the key the
        // building is observed as.
        Role::Ripperdoc => LocationKey::Clinic,
        Role::Mechanic => LocationKey::Garage,
        // M14 V16: a Lab's staff work at their employer (the wage desk).
        Role::Researcher => LocationKey::Workplace,
        // M15 W36: a Feed's staff likewise.
        Role::Reporter => LocationKey::Workplace,
        // L2 L2: the venues' and Fabs' staff likewise.
        Role::Host | Role::Attendant | Role::Cook | Role::Fighter | Role::Croupier | Role::Concierge | Role::Fabber => {
            LocationKey::Workplace
        }
        // M16a (plan C8): a Fixer office's staff likewise.
        Role::Fixer => LocationKey::Workplace,
        // Real economy E26: a Mission's Volunteers likewise.
        Role::Volunteer => LocationKey::Workplace,
        // Jobs and room J13: a trade's staff work at their employer.
        Role::Trade(_) => LocationKey::Workplace,
    }
}

/// L2 L8: the on-shift action of a role: `Sweep` for Sanitation with
/// `jobs::sweep_on`, else `ActionKind::work_for`.
pub fn work_action(world: &World, role: Role) -> ActionKind {
    if role == Role::Sanitation && crate::systems::jobs::sweep_on(world) {
        ActionKind::Sweep
    } else {
        ActionKind::work_for(role)
    }
}

/// M11 D13: the key of this agent's workplace: a guard employed at a
/// Security Office works at `Workplace`; every other role at its own kind.
pub fn workplace_key_for(world: &World, _agent: EntityId, job: &Job) -> LocationKey {
    if crate::systems::law::job_is_private_guard(world, job) {
        LocationKey::Workplace
    } else if job.role == Role::Sanitation && crate::systems::jobs::sweep_on(world) {
        // L2 L8: a sweeper's shift is on its beat.
        LocationKey::Beat
    } else {
        workplace_key(job.role)
    }
}

/// Is a shift key a working day (6 days in 7)?
pub fn is_workday(shift_key: i64) -> bool {
    shift_key.rem_euclid(7) != 6
}

/// `is_workday` for `id`'s `job`. L2 shadow fixes item 10: a guard's rest
/// day is staggered by its index (the whole watch shared the city's rest
/// day, so with Patrol and Arrest gated on the workday no guard worked one
/// day in seven: arrests on that weekday fell 8,226 -> 1,121 over 42-53).
pub fn workday_of(_world: &World, id: EntityId, job: &Job, shift_key: i64) -> bool {
    if job.role == Role::Guard {
        return (shift_key + i64::from(id.index % 7)).rem_euclid(7) != 6;
    }
    is_workday(shift_key)
}

/// A worked shift whose wages have not been collected today: the tail of the
/// Work plan (haul, Hall visit) is still pending.
pub fn wage_pending(world: &World, job: &Job) -> bool {
    job.last_shift_day.is_some() && job.wage_collectable(world.day())
}

/// A shift to attend to right now: on shift, waiting at the workplace for one
/// that starts soon, or time to set off.
pub fn shift_pending(world: &World, id: EntityId, job: &Job) -> bool {
    let key = job.next_shift_key(world.tick);
    if !workday_of(world, id, job, key) || job.last_shift_day == Some(key) {
        return false;
    }
    // A guard's Patrol day is the Patrol goal's business.
    if job.role == Role::Guard && !crate::systems::law::jail_duty(world, id, key) {
        return false;
    }
    let tod = world.tick_of_day();
    if job.on_shift(tod) {
        return true;
    }
    let at_work = job.employer.is_some() && world.comp::<Position>(id).is_some_and(|p| p.building == job.employer);
    let until = u64::from(job.ticks_until_shift(tod));
    // Round 3 S5 (OPEN-217): a one-day post (the dawn's hire, a sick
    // worker's cover) goes to the post as hired and waits there for the
    // shift: a day labourer picked at 06:00 scavenged the street until his
    // 08:00 shift (`day_labourer_75212 d25`).
    let day_post = job.until.is_some_and(|u| world.tick < u) && world.config.verbs.is_some();
    ((at_work || day_post) && until <= WAIT_AT_WORK_MAX) || must_leave_for_work(world, id)
}

/// The Work goal's gate: a shift to work, or wages to collect for one.
pub fn work_pending(world: &World, id: EntityId, job: &Job) -> bool {
    // L1: wages are paid at the shift's end; nothing to walk to off shift
    // (the Ripperdoc's Work scored ~0.95 off shift for a few coins owed).
    shift_pending(world, id, job)
}

/// The perf pass phase 1 (OPEN-195): the Rest loop's fix is on (`[rest]`;
/// absent, the base rule).
pub fn rest_fix(world: &World) -> bool {
    world.config.rest.is_some()
}

/// The perf pass phase 1 (OPEN-195): the Work goal's refusals that are not
/// the clock's: a sentence, a sick day (Hunger as sickness § 2), a week
/// unpaid (the `paid` gate). The Work goal's `in shift` gate reads it, and
/// with `[rest]` so does the departure gate, which is false while Work
/// refuses (it cut every Rest at a minute for a shift Work would not take).
pub fn work_refused(world: &World, id: EntityId, job: &Job) -> bool {
    world.has::<crate::components::Sentence>(id) || crate::systems::health::sick(world, id) || job.days_unpaid >= 7
}

/// The perf pass phase 1 (OPEN-195): a shift the Work goal would take now:
/// pending (a workday, not yet worked, a guard's jail day) and not refused.
pub fn work_due(world: &World, id: EntityId, job: &Job) -> bool {
    work_pending(world, id, job) && !work_refused(world, id, job)
}

/// Should this agent drop what it is doing and head to work now?
pub fn must_leave_for_work(world: &World, id: EntityId) -> bool {
    leave_gate_at(world, id, world.tick)
}

/// The departure gate with the clock at `at` (the body where it stands
/// now): `must_leave_for_work` at `world.tick`; at a resting body's Rest
/// start, whether the gate was already open when the Rest was chosen (a
/// resting body has not moved, and `transit::trip_due` reads the deadline's
/// trip, so the gate holds still while it waits).
pub fn leave_gate_at(world: &World, id: EntityId, at: crate::time::Tick) -> bool {
    let Some(job) = world.comp::<Job>(id) else { return false };
    if rest_fix(world) && work_refused(world, id, job) {
        return false;
    }
    let key = job.next_shift_key(at);
    if !workday_of(world, id, job, key) || job.last_shift_day == Some(key) {
        return false;
    }
    if job.role == Role::Guard && !crate::systems::law::jail_duty(world, id, key) {
        return false;
    }
    let Some(employer) = job.employer else { return false };
    let Some(pos) = world.comp::<Position>(id) else { return false };
    if pos.building == Some(employer) {
        return false;
    }
    let tod = (at % crate::time::TICKS_PER_DAY) as u16;
    let until = u64::from(job.ticks_until_shift(tod));
    let door = world.comp::<Building>(employer).map_or(pos.tile, |b| b.door);
    // L2 shadow fixes item 1: a commute under way holds until arrival (the
    // gate went false after a leg and Idle or Unwind took her home), and
    // the walk is read at the speed it is walked.
    if crate::systems::fixes::commuting(world, id, job) {
        return true;
    }
    // Transit T16: a rider leaves later (`transit::trip_due`: the trip as
    // it would be made to arrive by the margin).
    crate::systems::transit::trip_due_at(world, id, door, until, DEPARTURE_MARGIN, at)
}

/// Work before the shift starts: head to the workplace and rest inside until
/// it begins. `None` once the shift is on (the planner takes over) or when
/// nothing is pending.
pub fn commute_plan(world: &World, id: EntityId) -> Option<Plan> {
    let job = world.comp::<Job>(id)?;
    let employer = job.employer?;
    if job.on_shift(world.tick_of_day()) || !shift_pending(world, id, job) {
        return None;
    }
    let key = workplace_key_for(world, id, job);
    // Hunger fix item 11 (P5i): a meal before leaving, one packed to carry.
    let mut steps = pre_shift_meal(world, id, job);
    steps.extend([step(ActionKind::GoTo(key), Some(employer)), step(ActionKind::Rest, None)]);
    Some(plan(GoalKind::Work, Some(employer), steps, world.tick))
}

/// Hunger fix item 12 (P5j): ticks until bedtime before the next workday
/// shift (negative: past it): the leaving time (the shift's start less the
/// walk from the Home door to work and `DEPARTURE_MARGIN`) less the night
/// this body needs to wake full (`[needs] sleep_ticks_full` x the energy it
/// will lack at bedtime) less the walk home from here. `None` on
/// shift, on a rest day's eve, homeless or jobless, or with the item off.
pub fn ticks_to_bedtime(world: &World, id: EntityId) -> Option<i64> {
    if !crate::systems::fixes::work_night(world) {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    let tod = world.tick_of_day();
    if job.on_shift(tod) {
        return None;
    }
    let key = job.next_shift_key(world.tick);
    if !workday_of(world, id, job, key) || job.last_shift_day == Some(key) {
        return None;
    }
    let home = world.comp::<Household>(id).and_then(|h| h.home)?;
    let home_door = world.comp::<Building>(home)?.door;
    let work_door = world.comp::<Building>(job.employer?)?.door;
    let until = i64::from(job.ticks_until_shift(tod));
    // Transit T16: the trip from the Home door as the gate will read it (the
    // ride when the El wins, to arrive by the margin).
    let arrive_by = world.tick.saturating_add_signed(until - DEPARTURE_MARGIN as i64);
    let trip = crate::systems::transit::estimate_by(world, id, home_door, work_door, arrive_by) as i64;
    let leave = until - trip - DEPARTURE_MARGIN as i64;
    let at_home = world.comp::<Position>(id).is_some_and(|p| p.building == Some(home));
    let walk_home = if at_home { 0 } else { crate::systems::fixes::walk_estimate(world, id, home_door) as i64 };
    // The night this body needs: a full night (`sleep_ticks_full`) restores
    // energy 0 to 1, and Sleep ends at 1, so a night is `(1 - energy at
    // bedtime) x sleep_ticks_full`; the energy at bedtime is today's less
    // the decay until then (solved for the bedtime). A full 8 h from a
    // half-rested body put it to bed early and up at 04:00, out again
    // before the shift (measured on the probe: Unwind after midnight 34 ->
    // 929 minutes over twelve workers).
    let full = f64::from(world.config.needs.sleep_ticks_full);
    let e = f64::from(world.comp::<crate::components::Needs>(id).map_or(0.0, |n| n.energy));
    let d = f64::from(world.config.needs.energy_decay_per_tick);
    // bedtime b (ticks from now): leave - walk_home - b = (1 - (e - d b)) full
    //   => b (1 - d full) = leave - walk_home - (1 - e) full.
    let num = (leave - walk_home) as f64 - (1.0 - e).max(0.0) * full;
    let den = (1.0 - d * full).max(0.1);
    Some((num / den).round() as i64)
}

/// P5j: Sleep's work-night pressure, 0 at `work_night_ramp_ticks` before
/// bedtime rising to 1 at it (and after).
pub fn work_night_pressure(world: &World, id: EntityId) -> f32 {
    let Some(b) = ticks_to_bedtime(world, id) else { return 0.0 };
    let ramp = world.config.life.work_night_ramp_ticks.max(1) as f32;
    (1.0 - b as f32 / ramp).clamp(0.0, 1.0)
}

/// Shadow V4 bug 2: the small hours (the Night phase, 00:00-06:00) of a job
/// holder none of whose shifts runs in them: a day worker's night, for
/// sleep, not a corner or a grocery run.
pub fn day_worker_night(world: &World, id: EntityId) -> bool {
    world.phase() == crate::time::DayPhase::Night
        && world.comp::<Job>(id).is_some_and(|j| j.shifts.iter().all(|&(s, _)| s >= 360))
}

/// P5j: past bedtime on a work night (Unwind and the grocery run weigh less).
pub fn past_bedtime(world: &World, id: EntityId) -> bool {
    ticks_to_bedtime(world, id).is_some_and(|b| b <= 0)
}

/// P5i: ticks from now to the end of the next (or current) shift.
pub fn ticks_to_shift_end(world: &World, job: &Job) -> u64 {
    let until = u64::from(job.ticks_until_shift(world.tick_of_day()));
    job.shift_end(world.tick + until).map_or(until, |e| e.saturating_sub(world.tick))
}

/// P5i: the hunger `n` reaches by the next shift's end with nothing eaten.
fn hunger_at_shift_end(world: &World, hunger: f32, job: &Job) -> f32 {
    hunger - world.config.needs.hunger_decay_per_tick * ticks_to_shift_end(world, job) as f32
}

/// Hunger fix item 11 (a): at Home before a shift whose end finds the
/// worker under the Eat line (`HUNGER_SATISFIED`, below which Eat can win
/// on shift), with pantry meals: `EatAtHome` when a meal would not overfill
/// (hunger + `food_satisfy` <= 1), then `PackMeal` when the shift still
/// outlasts the hunger and nothing unstolen is carried.
fn pre_shift_meal(world: &World, id: EntityId, job: &Job) -> Vec<ActionInstance> {
    use crate::goap::world_state::HUNGER_SATISFIED;
    let mut steps = Vec::new();
    if !crate::systems::fixes::shift_meals(world) || job.on_shift(world.tick_of_day()) {
        return steps;
    }
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    if home.is_none() || world.comp::<Position>(id).map(|p| p.building) != Some(home) {
        return steps;
    }
    let Some(mut hunger) = world.comp::<crate::components::Needs>(id).map(|n| n.hunger) else { return steps };
    let mut pantry = home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food);
    let fs = world.config.needs.food_satisfy;
    if pantry > 0 && hunger_at_shift_end(world, hunger, job) < HUNGER_SATISFIED && hunger + fs <= 1.0 {
        steps.push(step(ActionKind::EatAtHome, home));
        hunger = (hunger + fs).min(1.0);
        pantry -= 1;
    }
    let carried = world.comp::<Inventory>(id).is_some_and(|i| i.food > i.stolen_food);
    if pantry > 0 && !carried && hunger_at_shift_end(world, hunger, job) < HUNGER_SATISFIED {
        steps.push(step(ActionKind::PackMeal, home));
    }
    steps
}

/// Hunger fix item 11 (b): the meals a shift takes, `ceil(drain over the
/// shift and an hour's walk each way ÷ food_satisfy)`, at least 1.
pub fn shift_meals_needed(world: &World, job: &Job) -> u32 {
    let len: u64 = job.shifts.iter().map(|&(s, e)| u64::from(e.saturating_sub(s))).max().unwrap_or(0) + 120;
    let drain = world.config.needs.hunger_decay_per_tick * len as f32;
    (drain / world.config.needs.food_satisfy.max(0.01)).ceil().max(1.0) as u32
}

/// Hunger fix item 11 (b): the meals the household holds toward a shift:
/// its Home pantry and the worker's own carried (not stolen) food.
fn meals_held(world: &World, id: EntityId, home: EntityId) -> u32 {
    let pantry = world.comp::<Building>(home).map_or(0, |b| b.stock_food);
    pantry + world.comp::<Inventory>(id).map_or(0, |i| i.food.saturating_sub(i.stolen_food))
}

/// Hunger fix item 11 (b): the pantry level a grocery run fills: a shift's
/// meals for each resident (5-6 share a Block's pantry; one worker's meal
/// was eaten by the rest), capped at the pantry.
fn household_need(world: &World, home: EntityId, job: &Job) -> u32 {
    let residents = world.residents_of(home).len().max(1) as u32;
    (shift_meals_needed(world, job) * residents).min(world.config.buildings.home.stock_cap)
}

/// Hunger fix item 11 (b): the meals a grocery run buys at the shop, the
/// household's shortfall (bounded by carry room, `FOOD_CAP`, the wallet and
/// the shelf); a person shops for what the week needs when they can carry
/// and pay for it, rather than three meals and back again.
pub fn grocery_cap(world: &World, id: EntityId) -> Option<u32> {
    let brain = world.comp::<crate::components::Brain>(id)?;
    let plan = brain.plan.as_ref().filter(|p| p.goal == GoalKind::Eat && p.steps.len() == 4)?;
    if plan.steps[1].action != ActionKind::BuyFood || plan.steps[3].action != ActionKind::StoreFood {
        return None;
    }
    let home = world.comp::<Household>(id).and_then(|h| h.home)?;
    let job = world.comp::<Job>(id)?;
    let short = household_need(world, home, job).saturating_sub(meals_held(world, id, home));
    Some(short.max(1))
}

/// Hunger fix item 11 (b): an off-shift worker's grocery run, `[GoTo(Market)
/// m, BuyFood m, GoTo(Home), StoreFood]`. `m` is the stocked Market with the
/// least detour (the walk from here to `m` and on home), where the worker
/// holds a meal's coins. The run is made when the household's pantry and the
/// worker's carried meals are under the pantry level `household_need`, if one
/// run can fill it (carry room, `FOOD_CAP`, the wallet and the shelf permit);
/// when it cannot (a Block's six residents), under the worker's own shift's
/// meals instead, so one worker does not shop twice a day for a pantry no
/// run can fill. It must bring home a meal for each hour of the round trip
/// (`[life] grocery_ticks_per_meal`), end before the worker must leave for
/// the shift, and end before bedtime and the energy line (`[life]
/// grocery_energy_floor`). A closed or packed Market is passed over.
pub fn grocery_plan(world: &World, id: EntityId) -> Option<Plan> {
    if !crate::systems::fixes::shift_meals(world) {
        return None;
    }
    let job = world.comp::<Job>(id)?;
    job.employer?;
    // Shadow V4 bug 2: no run out of bed in a day worker's small hours.
    if job.on_shift(world.tick_of_day()) || day_worker_night(world, id) {
        return None;
    }
    // The cheap shortfall test before any walk estimate.
    let home = world.comp::<Household>(id).and_then(|h| h.home)?;
    let home_door = world.comp::<Building>(home).filter(|b| !b.demolished)?.door;
    let held = meals_held(world, id, home);
    let need = household_need(world, home, job);
    if held >= need || must_leave_for_work(world, id) {
        return None;
    }
    // The market: least detour among the stocked ones it can buy at.
    let here = crate::exec::walk_origin(world, id)?;
    let short = need - held;
    let mut best: Option<(u64, EntityId, u32)> = None;
    for &m in world.buildings_of_kind(BuildingKind::Market) {
        let Some(b) = world.comp::<Building>(m).filter(|b| !b.demolished && !b.derelict && b.stock_food > 0) else {
            continue;
        };
        // Closed, or packed (a queue out of the door ends the errand with
        // `BuildingFull`): a shopper passes it over.
        if world.is_closed(m) || (b.is_full() && !world.capacity_exempt(id, m)) {
            continue;
        }
        let can = crate::systems::economy::buy_quantity_upto(world, id, Some(m), u32::MAX);
        if can == 0 {
            continue;
        }
        let detour =
            crate::exec::timed_walk(world, id, here, b.door) + crate::exec::timed_walk(world, id, b.door, home_door);
        if best.is_none_or(|(d, bm, _)| (detour, m) < (d, bm)) {
            best = Some((detour, m, can));
        }
    }
    let (detour, m, can) = best?;
    // One run cannot fill the household's pantry: the worker's own shift's meals.
    if can < short && held >= shift_meals_needed(world, job) {
        return None;
    }
    let trip = detour + 20;
    // The walk must be worth it: a meal brought home for each hour of it.
    // A worker who cannot pay for that many (and for less than the pantry
    // wants) stays home; one with food in the larder does not walk the
    // hours for a meal or two; one with an empty larder does.
    let per = world.config.life.grocery_ticks_per_meal;
    if per > 0 {
        let want = trip.div_ceil(per) as u32;
        if can < want.min(short) || (held > 0 && short < want) {
            return None;
        }
    }
    // The run must end before the worker must leave for the shift ...
    let employer_door = job.employer.and_then(|e| world.comp::<Building>(e)).map(|b| b.door)?;
    let to_work = crate::exec::timed_walk(world, id, home_door, employer_door);
    let until = u64::from(job.ticks_until_shift(world.tick_of_day()));
    if trip + to_work + 2 * DEPARTURE_MARGIN > until {
        return None;
    }
    // ... before bedtime (home by the time the night's sleep must begin) ...
    if let Some(b) = ticks_to_bedtime(world, id) {
        let walk_home = if world.comp::<Position>(id).is_some_and(|p| p.building == Some(home)) {
            0
        } else {
            crate::exec::timed_walk(world, id, here, home_door) as i64
        };
        if trip as i64 > b + walk_home {
            return None;
        }
    }
    // ... and before the body is tired enough that Sleep wins mid-walk.
    let decay = world.config.needs.energy_decay_per_tick;
    let energy = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.energy);
    if energy - decay * (trip as f32) < world.config.life.grocery_energy_floor {
        return None;
    }
    let steps = vec![
        step(ActionKind::GoTo(LocationKey::Market), Some(m)),
        step(ActionKind::BuyFood, Some(m)),
        step(ActionKind::GoTo(LocationKey::Home), Some(home)),
        step(ActionKind::StoreFood, None),
    ];
    Some(plan(GoalKind::Eat, Some(m), steps, world.tick))
}

/// Fed past the Eat line (`HUNGER_SATISFIED`): an Eat goal is then the
/// grocery run's.
pub fn fed(world: &World, id: EntityId) -> bool {
    use crate::goap::world_state::HUNGER_SATISFIED;
    world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger >= HUNGER_SATISFIED)
}

/// Hunger fix item 11 (b): a fed worker's Eat goal stands for the grocery
/// run (`grocery_plan` would plan one now).
pub fn grocery_due(world: &World, id: EntityId) -> bool {
    fed(world, id) && grocery_plan(world, id).is_some()
}

/// Idle bypasses the planner: Rest at Home or the Bar; otherwise go home
/// (loitering inside the Hall or Market blocks everyone else), or Wander
/// outside if homeless. Spare food goes into the pantry first.
/// M12 phase 4: an expedition's plan is always the same chain, so it is
/// built directly (as the commute and the idle plan are): `GoTo(MusterPoint)
/// → Muster → GoTo(RaidTarget) → Brawl`, the steps already behind the agent
/// dropped. The planner's 200-expansion cap could not reach a door 200
/// tiles off past the cheap GoTos (`ExpansionCap`), so far members never
/// answered a muster. `None` without a pending expedition.
pub fn raid_plan(world: &World, id: EntityId) -> Option<Plan> {
    use crate::systems::raid;
    if !raid::raid_pending(world, id) || raid::raid_done(world, id) {
        return None;
    }
    let target = raid::target_building(world, id);
    let ws = crate::goap::WorldState::observe(world, id, target);
    let mustered = raid::mustered(world, id);
    let mut steps = Vec::new();
    if !mustered {
        raid::muster_point(world, id)?;
        if ws.at != LocationKey::MusterPoint {
            steps.push(step(ActionKind::GoTo(LocationKey::MusterPoint), None));
        }
        steps.push(step(ActionKind::Muster, None));
    }
    raid::target_tile(world, id)?;
    if !mustered || ws.at != LocationKey::RaidTarget {
        steps.push(step(ActionKind::GoTo(LocationKey::RaidTarget), None));
    }
    steps.push(step(ActionKind::Brawl, None));
    Some(plan(GoalKind::Raid, target, steps, world.tick))
}

pub fn idle_plan(world: &World, id: EntityId) -> Option<Plan> {
    let tick = world.tick;
    let pos = world.comp::<Position>(id)?;
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let inv = world.comp::<Inventory>(id).cloned().unwrap_or_default();
    let at_home = home.is_some() && pos.building == home;
    let goal = GoalKind::Idle;
    if at_home && inv.food > inv.stolen_food && inv.food >= 2 {
        return Some(plan(goal, home, vec![step(ActionKind::StoreFood, None)], tick));
    }
    // Hunger fix item 11 (b): the grocery run (as an Eat plan).
    if let Some(mut p) = grocery_plan(world, id) {
        p.goal = goal;
        return Some(p);
    }
    // A member lying low, or without a Home, idles at the Hideout.
    if let Some(h) = crate::systems::gang::holes_up_at(world, id) {
        if pos.building == Some(h) {
            return Some(plan(goal, None, vec![step(ActionKind::Rest, None)], tick));
        }
        let steps = vec![step(ActionKind::GoTo(LocationKey::Hideout), Some(h)), step(ActionKind::Rest, None)];
        return Some(plan(goal, Some(h), steps, tick));
    }
    let here = pos.building.and_then(|b| world.comp::<Building>(b)).map(|b| b.kind);
    // L1: idle at Home at night is bed, not a chair (90-minute Rests kept
    // energy up, so Sleep never scored and the night was spent sitting).
    let night = world.phase() == crate::time::DayPhase::Night || world.tick_of_day() >= 22 * 60;
    let off_shift = world.comp::<Job>(id).is_none_or(|j| !j.on_shift(world.tick_of_day()));
    // L2 shadow fixes item 3: not above `idle_sleep_energy` (a guard logged
    // ten 1-6 min Sleeps at energy 0.89-0.97 from this plan).
    let idle_sleep_below = world.config.life.idle_sleep_energy;
    if at_home
        && night
        && off_shift
        && world.comp::<crate::components::Needs>(id).is_some_and(|n| n.energy < idle_sleep_below)
    {
        return Some(plan(goal, None, vec![step(ActionKind::Sleep, None)], tick));
    }
    if at_home || here == Some(BuildingKind::Bar) {
        return Some(plan(goal, None, vec![step(ActionKind::Rest, None)], tick));
    }
    // L1: far from Home, idle at the nearer Bar (or a member's Hideout)
    // rather than walk hours home to sit down.
    if let Some(plan) = idle_near(world, id, home) {
        return Some(plan);
    }
    if home.is_some() {
        let steps = vec![step(ActionKind::GoTo(LocationKey::Home), None), step(ActionKind::Rest, None)];
        return Some(plan(goal, home, steps, tick));
    }
    Some(plan(goal, None, vec![step(ActionKind::Wander, None)], tick))
}

/// L1: a housed idle agent past `[life] near_tiles` from Home does not walk
/// hours home to sit down (26 h of the shadowed week): a member rests at
/// its Hideout when that is nearer by `bed_margin_tiles`; anyone else
/// idles where they are.
fn idle_near(world: &World, id: EntityId, home: Option<EntityId>) -> Option<Plan> {
    use crate::systems::life;
    let home_t = home.and_then(|h| life::tiles_to(world, id, h))?;
    // OPEN-140: a body moved in walks to its new Home (`idle_plan`'s GoTo).
    if home_t <= world.config.life.idle_far_tiles || life::moving_in(world, id).is_some() {
        return None;
    }
    let margin = world.config.life.bed_margin_tiles;
    if let Some(h) = world.gang_of(id).and_then(|g| world.hideout_of(g)) {
        if life::tiles_to(world, id, h).is_some_and(|t| t + margin <= home_t) {
            let steps = vec![step(ActionKind::GoTo(LocationKey::Hideout), Some(h)), step(ActionKind::Rest, None)];
            return Some(plan(GoalKind::Idle, Some(h), steps, world.tick));
        }
    }
    Some(plan(GoalKind::Idle, None, vec![step(ActionKind::Wander, None)], world.tick))
}
