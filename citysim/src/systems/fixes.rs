//! L2 shadow fixes (docs/SHADOW_V2.md, "L2 shadow fixes (what landed)"):
//! the helpers the 21 `[bug]` items of the V2 re-shadow share (and item 22,
//! the gravedigger's burial shift, added 2026-10-08 with addendum 17). Every caller
//! reads `on` (or `item`, or a sub-switch through it) first: with `[life]
//! l2_fixes = false` (`--l2-off`, `--life-off`) or `[living] enabled = false`
//! nothing here runs, and no item draws a roll of its own on any stream.
//! `CITYSIM_L2FIX_OFF=1,7` leaves single items off (a bisection device).

use crate::components::{Brain, Building, GoalKind, Job, Lod, Needs, Position, Role, Sentence, TilePos};
use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::goap::ActionKind;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// Are the L2 shadow fixes on (`[life] enabled && l2_fixes`, and the L2
/// master `[living] enabled`: the L2 gate's "the master alone reproduces
/// `living_off`")?
pub fn on(world: &World) -> bool {
    world.config.life.enabled && world.config.life.l2_fixes && world.config.living.enabled
}

/// Diagnostics: items to leave off, from `CITYSIM_L2FIX_OFF` (a comma
/// list of item numbers), read once. Unset (every run but a bisection):
/// none.
fn off_mask() -> u32 {
    static MASK: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *MASK.get_or_init(|| {
        std::env::var("CITYSIM_L2FIX_OFF")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.trim().parse::<u32>().ok()).fold(0, |m, n| m | (1 << n.min(31))))
            .unwrap_or(0)
    })
}

/// Is fix item `n` (the SHADOW_V2 Plan's numbering) on?
pub fn item(world: &World, n: u32) -> bool {
    on(world) && off_mask() & (1 << n.min(31)) == 0
}

/// Item 1: the commute latch and the real-speed walk estimate.
pub fn commute_latch(world: &World) -> bool {
    item(world, 1) && world.config.life.commute_latch
}

/// Item 2: the shift commitment and pro-rata pay.
pub fn shift_commit(world: &World) -> bool {
    item(world, 2) && world.config.life.shift_commit
}

/// Item 3: the committed sleep, the energy brake, Idle's Sleep cap.
pub fn sleep_commit(world: &World) -> bool {
    item(world, 3) && world.config.life.sleep_commit
}

/// Item 7: the LOD tier dwell.
pub fn lod_dwell(world: &World) -> bool {
    item(world, 7) && world.config.life.lod_dwell
}

/// Item 8: Statistical Sleep when the hungry hour's Eat fails.
pub fn stat_sleep(world: &World) -> bool {
    item(world, 8) && world.config.life.stat_sleep_futile_eat
}

/// Item 19: Beg at a HangOut spot with company.
pub fn beg_at_spot(world: &World) -> bool {
    item(world, 19) && world.config.life.beg_at_spot
}

// ---------------------------------------------------------------------------
// The violence fixes (2026-10-09, the wages-on diagnosis)
// ---------------------------------------------------------------------------

/// Are the violence fixes on (`[life] enabled && violence_fixes`, and the
/// L2 master `[living] enabled`: the L2 gate's "the master alone
/// reproduces `living_off`")? Off (`--l2-off`, `--life-off`) nothing they
/// gate runs.
pub fn violence_on(world: &World) -> bool {
    world.config.life.enabled && world.config.life.violence_fixes && world.config.living.enabled
}

/// Diagnostics: violence-fix items to leave off, from `CITYSIM_VFIX_OFF`
/// (a comma list of item numbers), read once.
fn vfix_off_mask() -> u32 {
    static MASK: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *MASK.get_or_init(|| {
        std::env::var("CITYSIM_VFIX_OFF")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.trim().parse::<u32>().ok()).fold(0, |m, n| m | (1 << n.min(31))))
            .unwrap_or(0)
    })
}

/// Is violence-fix item `n` on?
pub fn vfix_item(world: &World, n: u32) -> bool {
    violence_on(world) && vfix_off_mask() & (1 << n.min(31)) == 0
}

/// Violence fix 1: the ledger's `VictimClass::Street`.
pub fn street_class(world: &World) -> bool {
    vfix_item(world, 1) && world.config.life.vf_street_class
}

/// Violence fix 2: short sources (riots, episodes) roll for their live hours.
pub fn short_sources(world: &World) -> bool {
    vfix_item(world, 2) && world.config.life.vf_short_sources
}

/// Violence fix 3: the chrome humanity cap and the fighter list by role.
pub fn chrome_cap(world: &World) -> bool {
    vfix_item(world, 3) && world.config.life.vf_chrome_cap
}

/// Violence fix 5: an immigrant arrives with a seeded adult's savings.
pub fn arrival(world: &World) -> bool {
    vfix_item(world, 5) && world.config.life.vf_arrival
}

/// Violence fix 4: rent from income leaves an agent in arrears a meal.
pub fn rent_meal(world: &World) -> bool {
    vfix_item(world, 4) && world.config.life.vf_rent_meal
}

// ---------------------------------------------------------------------------
// Items 1-4, 15: what a goal change may not abort
// ---------------------------------------------------------------------------

/// Item 1: a walk at the timed (real) speed: `exec::timed_walk`, the walk a
/// Coarse body takes and a Full one takes at about the same rate (the
/// `commute_tpt_walk` column reads ~2.05 ticks a tile against
/// `move_ticks_full` 2). The old estimate padded by half and so shrank 1.5x
/// faster than the clock: the leave-for-work gate went false after a leg.
pub fn walk_estimate(world: &World, id: EntityId, to: TilePos) -> Tick {
    let from = crate::exec::walk_origin(world, id).unwrap_or(to);
    crate::exec::timed_walk(world, id, from, to)
}

/// Item 1: `id` is on its Work plan for `job`'s employer, made for the
/// shift still pending (the plan's shift key is the one due now).
pub fn commuting(world: &World, id: EntityId, job: &Job) -> bool {
    let Some(employer) = job.employer else { return false };
    let Some(p) = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()) else { return false };
    p.goal == GoalKind::Work
        && p.target == Some(employer)
        && job.next_shift_key(p.started_tick) == job.next_shift_key(world.tick)
        && job.last_shift_day != Some(job.next_shift_key(world.tick))
}

/// Starvation or danger: the winners a commitment gives way to.
pub fn emergency(world: &World, id: EntityId, winner: GoalKind) -> bool {
    match winner {
        GoalKind::Flee | GoalKind::Fight | GoalKind::Arrest => true,
        GoalKind::Eat => world.comp::<Needs>(id).is_some_and(|n| n.hunger < world.config.life.starving_hunger),
        _ => false,
    }
}

/// A purchase under way (item 4): the coins moved at its start.
pub fn paid_step(kind: ActionKind) -> bool {
    matches!(kind, ActionKind::BuyFood | ActionKind::EatOut | ActionKind::Enjoy | ActionKind::Gamble)
}

/// Items 1-4 and 15: may `winner` not take over from `brain`'s plan? A
/// latched commute (or an exec's office walk), an in-shift work step, a
/// Sleep, a paid step and a started Lead hold against anything but an
/// emergency.
pub fn committed(world: &World, id: EntityId, brain: &Brain, winner: GoalKind) -> bool {
    if !on(world) || brain.current_goal == Some(winner) || emergency(world, id, winner) {
        return false;
    }
    let Some(plan) = brain.plan.as_ref() else { return false };
    if let ExecState::Use { kind, .. } = &brain.exec {
        if shift_commit(world) && kind.is_work() && plan.goal == GoalKind::Work {
            return true;
        }
        if sleep_commit(world) && *kind == ActionKind::Sleep {
            return true;
        }
        if item(world, 4) && paid_step(*kind) {
            return true;
        }
    }
    if commute_latch(world) && plan.goal == GoalKind::Work {
        match world.comp::<Job>(id) {
            Some(job) => {
                if commuting(world, id, job) {
                    return true;
                }
            }
            // The exec's walk to the office (`life::exec_plan`).
            None => {
                if plan.target.is_some() && crate::systems::life::exec_day_pending(world, id) == plan.target {
                    return true;
                }
            }
        }
    }
    // Item 15: a Lead holds until its Collect runs or fails.
    item(world, 15) && plan.goal == GoalKind::Lead
}

/// Item 2: the length of `job`'s shift in ticks (the guards' rule,
/// `law::credit_guard_shifts`: the segments summed).
fn shift_length(job: &Job) -> u64 {
    job.shifts.iter().map(|&(s, e)| u64::from(e.saturating_sub(s))).sum()
}

/// Item 2: a work step aborted after `pro_rata` of the shift or more is a
/// worked shift paid for the share worked (`economy::collect_wage_scaled`).
/// A guard is paid by the shift clock already.
pub fn work_aborted(world: &mut World, id: EntityId, kind: ActionKind, started: Tick) {
    if !shift_commit(world) || !kind.is_work() {
        return;
    }
    let Some(job) = world.comp::<Job>(id) else { return };
    if job.role == Role::Guard {
        return;
    }
    let key = job.shift_key_at(started);
    if job.last_shift_day == Some(key) {
        return;
    }
    let length = shift_length(job).max(1);
    let worked = world.tick.saturating_sub(started);
    let share = (worked as f32 / length as f32).min(1.0);
    if share < world.config.life.shift_pro_rata {
        return;
    }
    let mut owed_before = 0.0;
    if let Some(j) = world.comp_mut::<Job>(id) {
        owed_before = f32::from(j.days_unpaid);
        j.last_shift_day = Some(key);
        j.days_unpaid = j.days_unpaid.saturating_add(1);
    }
    // Arrears are owed in full; only today's day is cut to the share.
    let scale = (owed_before + share) / (owed_before + 1.0);
    crate::systems::economy::collect_wage_scaled(world, id, scale);
    world.stats.current.living.fix_pro_rata += 1;
}

// ---------------------------------------------------------------------------
// Item 7: the LOD dwell
// ---------------------------------------------------------------------------

/// Item 7: a body promoted or demoted less than `lod_dwell_ticks` ago (to
/// the plan's end, at most twice that), or mid-purchase, keeps its tier
/// against an equal-priority newcomer.
pub fn dwell_holds(world: &World, id: EntityId) -> bool {
    let Some(b) = world.comp::<Brain>(id) else { return false };
    if b.lod == Lod::Statistical {
        return false;
    }
    if matches!(b.exec, ExecState::Use { kind, .. } if paid_step(kind)) {
        return true;
    }
    let dwell = world.config.life.lod_dwell_ticks;
    let since = world.tick.saturating_sub(b.lod_since);
    since < dwell || (b.plan.is_some() && since < 2 * dwell)
}

/// Item 7: `id`'s tier changed less than `lod_dwell_ticks` ago.
pub fn dwell_recent(world: &World, id: EntityId) -> bool {
    world
        .comp::<Brain>(id)
        .is_some_and(|b| b.lod_since > 0 && world.tick.saturating_sub(b.lod_since) < world.config.life.lod_dwell_ticks)
}

// ---------------------------------------------------------------------------
// Item 11: absentees
// ---------------------------------------------------------------------------

/// Item 11: the working shifts `job` has missed: the workday keys after the
/// last one worked (or the hire) and before the shift in progress or next
/// (a shift is missed once it is over). 0 for a guard (paid by the clock),
/// a struck shift, or with the item off.
pub fn missed_workdays(world: &World, job: &Job) -> i64 {
    if !item(world, 11) || job.role == Role::Guard {
        return 0;
    }
    let current = job.next_shift_key(world.tick);
    let hired = job.shift_key_at(job.hired_tick);
    let from = job.last_shift_day.unwrap_or(hired - 1).max(hired - 1);
    (from + 1..current).filter(|&k| crate::exec::routine::is_workday(k) && job.struck_shift != Some(k)).count() as i64
}

// ---------------------------------------------------------------------------
// Item 22: a burial is the gravedigger's shift
// ---------------------------------------------------------------------------

/// Item 22: a Gravedigger's completed burial (`BuryCorpse`) on a workday
/// whose shift is not yet worked counts as that day's worked shift, paid
/// as a shift's end pays (`economy::collect_wage`). Wages only accrued on
/// the `TendGraves` shift, which a digger out burying the day's dead missed
/// every 1-5 days: on 42-44, with the unpaid-worker dole gone (addendum
/// 17), six of seven starved were gravediggers with `days_unpaid` 0.
pub fn burial_shift(world: &mut World, id: EntityId) {
    if !item(world, 22) {
        return;
    }
    let Some(job) = world.comp::<Job>(id) else { return };
    if job.role != Role::Gravedigger {
        return;
    }
    let key = job.shift_key_at(world.tick);
    if job.last_shift_day.is_some_and(|d| d >= key) || !crate::exec::routine::workday_of(world, id, job, key) {
        return;
    }
    if let Some(j) = world.comp_mut::<Job>(id) {
        j.last_shift_day = Some(key);
        j.days_unpaid = j.days_unpaid.saturating_add(1);
    }
    crate::systems::economy::collect_wage(world, id);
}

// ---------------------------------------------------------------------------
// The midnight pass (items 11, 14, 17)
// ---------------------------------------------------------------------------

/// Midnight, in `living::run`: the no-show dismissals (item 11), the
/// seeded sweepers onto the works wage (item 14), the told-memory pruned
/// (item 17).
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    // Item 11: a week of workdays missed ends the job (a Feed's reporter
    // never reached her desk and was never paid or dismissed; the absentee
    // never draws the dole: addendum 17).
    let fire = world.config.life.noshow_fire_days;
    let mut out: Vec<EntityId> = Vec::new();
    // scan-ok: daily: no-show dismissals
    for id in world.citizens().into_iter().filter(|_| item(world, 11)) {
        let Some(job) = world.comp::<Job>(id) else { continue };
        if world.has::<Sentence>(id) || job.employer.is_none() {
            continue;
        }
        if missed_workdays(world, job) >= fire {
            out.push(id);
        }
    }
    for id in out {
        // The job stays (a vacancy for someone who turns up), as a quit's.
        world.abort_plan(id);
        let Some(job) = world.vacate_job(id) else { continue };
        // Not rehired there for `quit_rehire_days` (`pick_candidate`'s
        // `quit_blocks`): else the absentee was rehired next morning.
        crate::systems::life::note_quit(world, id, job.employer);
        let text = format!("{} dismissed as {} (no-show)", world.name_of(id), job.role.label());
        let mut actors = vec![id];
        actors.extend(job.employer);
        world.push_event(crate::events::EventKind::Fire, &actors, text);
        world.stats.current.living.fix_noshow += 1;
    }
    // Item 14: every sweeper of the Recycler on the works wage (the seeded
    // ones kept `wage_sanitation` 5, under the dole after tax).
    let works = world.config.budget.works_wage;
    if works > 0 && world.config.budget.enabled && item(world, 14) {
        if let Some(recycler) = world.building_of_kind(crate::components::BuildingKind::Cemetery) {
            for s in world.workers(Role::Sanitation).to_vec() {
                if let Some(j) = world.comp_mut::<Job>(s) {
                    if j.employer == Some(recycler) && j.wage_per_day < works {
                        j.wage_per_day = works;
                    }
                }
            }
        }
    }
    // Item 17: tellings past the cooldown are forgotten.
    let keep = world.config.life.told_cooldown_days * TICKS_PER_DAY;
    let now = world.tick;
    world.told.retain(|_, &mut t| now.saturating_sub(t) < keep);
}

// ---------------------------------------------------------------------------
// Item 12: the Clinic's counter
// ---------------------------------------------------------------------------

/// Item 12: a staff member on a working shift at the counter (a body in
/// the building, or a Statistical one credited by the clock).
pub fn staff_present(world: &World, b: EntityId, role: Role) -> bool {
    let tod = world.tick_of_day();
    world.workers(role).iter().any(|&w| {
        world.comp::<Job>(w).is_some_and(|j| {
            j.employer == Some(b)
                && j.on_shift(tod)
                && crate::exec::routine::is_workday(j.shift_key_at(world.tick))
                && !world.has::<Sentence>(w)
                && world.comp::<Brain>(w).is_some_and(|br| {
                    br.lod == Lod::Statistical || world.comp::<Position>(w).is_some_and(|p| p.building == Some(b))
                })
        })
    })
}

// ---------------------------------------------------------------------------
// Item 5, 6: the spot pick
// ---------------------------------------------------------------------------

/// Item 6: `id` avoids the spot at `t` (within 3 tiles of a tile it fled).
pub fn avoided(world: &World, id: EntityId, t: TilePos) -> bool {
    world
        .comp::<Brain>(id)
        .and_then(|b| b.avoid_spot)
        .is_some_and(|(at, until)| until > world.tick && at.manhattan(t) <= 3)
}

/// Item 5: the bodies on `id`'s Enemy edges, where they stand.
pub fn enemy_tiles(world: &World, id: EntityId) -> Vec<TilePos> {
    world
        .enemies_of(id)
        .filter(|&o| world.has::<Brain>(o))
        .filter_map(|o| world.comp::<Position>(o).filter(|p| p.building.is_none()).map(|p| p.tile))
        .collect()
}

/// Item 5: how many of `enemies` stand within 3 tiles of `t`.
pub fn enemies_near(enemies: &[TilePos], t: TilePos) -> usize {
    enemies.iter().filter(|&&e| e.manhattan(t) <= 3).count()
}

/// Item 19: `id` stands on the street at a HangOut spot where someone
/// else hangs out (within 2 tiles).
pub fn at_spot_with_company(world: &World, id: EntityId) -> bool {
    let Some(p) = world.comp::<Position>(id).filter(|p| p.building.is_none()) else { return false };
    world.hangouts.iter().any(|(t, v)| t.manhattan(p.tile) <= 2 && v.iter().any(|&o| o != id))
}

/// Item 10: the guard-prisoner pair in the cells (one employed at the
/// building, the other serving a sentence).
pub fn guard_and_prisoner(world: &World, a: EntityId, b: EntityId, building: EntityId) -> bool {
    let staff = |x: EntityId| world.comp::<Job>(x).is_some_and(|j| j.employer == Some(building));
    (staff(a) && world.has::<Sentence>(b)) || (staff(b) && world.has::<Sentence>(a))
}

/// Item 16: a Deal shift may start now (`deal_busy_from`..`deal_busy_to`,
/// wrapping past midnight).
pub fn deal_hours(world: &World) -> bool {
    let tod = world.tick_of_day();
    let (from, to) = (world.config.life.deal_busy_from, world.config.life.deal_busy_to);
    if from <= to {
        tod >= from && tod < to
    } else {
        tod >= from || tod < to
    }
}

/// Item 15: the farthest body of `gang`'s walk to its Hideout, capped at
/// `muster_walk_cap_ticks`.
pub fn muster_walk(world: &World, gang: EntityId) -> Tick {
    let Some(door) = world.hideout_of(gang).and_then(|h| world.comp::<Building>(h)).map(|b| b.door) else {
        return 0;
    };
    let far = crate::systems::leisure::members(world, gang)
        .into_iter()
        .filter(|&m| world.comp::<Brain>(m).is_some_and(|b| b.lod != Lod::Statistical))
        .filter(|&m| !world.has::<Sentence>(m))
        .map(|m| walk_estimate(world, m, door))
        .max()
        .unwrap_or(0);
    far.min(world.config.life.muster_walk_cap_ticks)
}

/// Item 17: the told-memory key of one telling.
pub fn told_key(from: EntityId, to: EntityId, r: &crate::word::DeedRef, day: u64) -> (EntityId, EntityId, u64) {
    let a = r.actor.map_or(u64::from(u32::MAX), |x| u64::from(x.index));
    let o = r.object.map_or(u64::from(u32::MAX), |x| u64::from(x.index));
    let h = crate::rng::splitmix64((r.deed as u64) ^ (a << 8) ^ (o << 36) ^ day.rotate_left(20));
    (from, to, h)
}
