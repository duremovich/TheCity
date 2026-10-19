//! Lives at work phase 6: ages, careers and leaders (docs/LIVES_AT_WORK.md
//! § 4; D9, D10; Dylan's answers 2, 3 and 6). Two switches under the
//! `[lives]` master:
//!
//! - **`[careers] enabled`** (`CITYSIM_CAREERS_OFF=1`, § 4.1-4.4). Age bands
//!   (`[[trades]] min_age`/`max_age`, `[roles.<Name>]`) are a hard gate on a
//!   new hire, never a firing ([`admits`], read by every hiring pick and by
//!   the seeded jobs). A seeded Job carries `seniority_days` ([`seed`]);
//!   experience is that plus the days since the hire, and the foreman is the
//!   most experienced worker aged `foreman_min_age` or more ([`foreman`]),
//!   who draws `foreman_premium` (`Job.foreman`, set at midnight) and is an
//!   actor of the building's `Hire` and `Layoff` events. The `Profession`
//!   is written at each worked shift in both tiers ([`shift_end`]) and ranks
//!   a holder first for the trade's vacancies. At midnight ([`daily`]) a
//!   worker past `retire_from_years` retires to an adult child's Home (the
//!   `[ages]` kin take-in, first) or on savings; a worker whose
//!   `ages::work_mult` falls under `frail_exit_mult` is let go "too frail".
//! - **`[gangs] lieutenants`** (`CITYSIM_LIEUTENANTS_OFF=1`, § 4.5). Rank 1
//!   for the lieutenants (`gang::recompute_leader`), the leader's
//!   delegation ([`delegating`]: `gang::following_order`,
//!   `stat_extort_target`, `raid::raid_pending`), the raid's head
//!   ([`raid_head`]) and its ladder ([`credit_raid`]), the payout's cut
//!   (`leisure::payout`).
//!
//! Every rule is world-side (hire rules, midnight, the payout, the raid's
//! resolution), so a Statistical and a Full agent read the same (§ 6). Every
//! coin moves between purses. No stream draw: the seniority's `u` is a hash.

use std::collections::BTreeSet;

use crate::components::{Gang, GangMember, Job, Profession, Retired, Role, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership;
use crate::time::{DAYS_PER_YEAR, TICKS_PER_DAY};
use crate::world::World;

const PURPOSE_TENURE: u64 = 0x4352_0001;
const PURPOSE_RETIRE_KIN: u64 = 0x4352_0002;

/// § 4.1-4.4 are live: `[lives] enabled` and `[careers] enabled`.
pub fn on(world: &World) -> bool {
    on_cfg(&world.config)
}

/// [`on`] from a config.
pub fn on_cfg(cfg: &crate::config::Config) -> bool {
    cfg.lives.enabled && cfg.careers.enabled
}

/// § 4.5 is live: `[lives] enabled` and `[gangs] lieutenants`.
pub fn lieutenants_on(world: &World) -> bool {
    world.config.lives.enabled && world.config.gangs.lieutenants
}

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

fn years(world: &World, id: EntityId) -> u32 {
    crate::systems::ages::years(world, id)
}

// ---------------------------------------------------------------------------
// § 4.1 The bands
// ---------------------------------------------------------------------------

/// A role's band `(min_age, max_age)` in years (0 none): its trade row's,
/// else its `[roles.<Name>]` table's.
pub fn band_cfg(cfg: &crate::config::Config, role: Role) -> (u32, u32) {
    if let Some(r) = role.trade().and_then(|t| cfg.trade(t)) {
        return (r.min_age, r.max_age);
    }
    if role.is_trade() {
        return (0, 0);
    }
    cfg.roles.get(&format!("{role:?}")).map_or((0, 0), |r| (r.min_age, r.max_age))
}

/// The hiring gate for `role`: its band with careers on, `None` off (every
/// candidate admitted). Read once a pick, then [`admits`] per candidate.
pub fn hire_band(world: &World, role: Role) -> Option<(u32, u32)> {
    on(world).then(|| band_cfg(&world.config, role))
}

/// `years` within `(lo, hi)` (0 none).
pub fn fits(years: u32, (lo, hi): (u32, u32)) -> bool {
    (lo == 0 || years >= lo) && (hi == 0 || years <= hi)
}

/// Why a candidate fails the gate: `None` admitted, `Some(true)` for age
/// (a band refusal, or too frail: § 4.4's exit is not undone by the next
/// hire), `Some(false)` retired.
pub fn refusal(world: &World, id: EntityId, band: Option<(u32, u32)>) -> Option<bool> {
    let b = band?;
    if world.has::<Retired>(id) {
        return Some(false);
    }
    let y = years(world, id);
    (!fits(y, b) || frail(world, id, y)).then_some(true)
}

/// § 4.4: `id` (aged `y`) is past `[ages] frail_from_years` and works at
/// under `frail_exit_mult` (`ages::work_mult`; never with `[ages]` off).
fn frail(world: &World, id: EntityId, y: u32) -> bool {
    crate::systems::ages::on(world)
        && y >= world.config.ages.frail_from_years
        && crate::systems::ages::work_mult(world, id) < world.config.careers.frail_exit_mult
}

/// The gate: `id` may be hired into a role of `band` (careers off: always).
pub fn admits(world: &World, id: EntityId, band: Option<(u32, u32)>) -> bool {
    refusal(world, id, band).is_none()
}

/// [`admits`] for `role`.
pub fn age_ok(world: &World, id: EntityId, role: Role) -> bool {
    admits(world, id, hire_band(world, role))
}

// ---------------------------------------------------------------------------
// § 4.2 Seniority, experience and the foreman
// ---------------------------------------------------------------------------

/// `id`'s experience in days: its Job's seniority plus the days since the hire.
pub fn experience(world: &World, id: EntityId) -> u32 {
    world
        .comp::<Job>(id)
        .map_or(0, |j| u32::from(j.seniority_days) + (world.tick.saturating_sub(j.hired_tick) / TICKS_PER_DAY) as u32)
}

/// `World::new`, last: every seeded Job's seniority, `min(years over the
/// row's min_age (adulthood when none), seniority_cap_years) ×
/// DAYS_PER_YEAR × hash(PURPOSE_TENURE, id)` days, and its `Profession`
/// when that reaches `profession_min_days`. No stream draw.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.careers.clone();
    let adult = crate::systems::demography::ADULT_AGE_DAYS / DAYS_PER_YEAR as u32;
    for id in world.citizens() {
        let Some(role) = world.comp::<Job>(id).map(|j| j.role) else { continue };
        let lo = band_cfg(&world.config, role).0.max(adult);
        let over = years(world, id).saturating_sub(lo).min(cfg.seniority_cap_years);
        let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_TENURE, key(id), 0);
        let days = ((over as u64 * DAYS_PER_YEAR) as f32 * u).floor().min(f32::from(u16::MAX)) as u16;
        if let Some(j) = world.comp_mut::<Job>(id) {
            j.seniority_days = days;
        }
        if u32::from(days) >= cfg.profession_min_days {
            world.insert(id, Profession { trade: role, days: u32::from(days), last_day: 0, next: None });
        }
    }
}

/// § 4.2: the foreman of `b`: the most experienced of its staff aged
/// `foreman_min_age` or more (ties the row's skill, then the lower id),
/// `exclude` left out. `None` when nobody qualifies.
pub fn foreman(world: &World, b: EntityId, exclude: Option<EntityId>) -> Option<EntityId> {
    let min = world.config.careers.foreman_min_age;
    world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| Some(s) != exclude && years(world, s) >= min)
        .filter_map(|s| {
            let j = world.comp::<Job>(s)?;
            let skill = crate::systems::competence::role_skill(world, s, j.role).map_or(0.0, |(v, _, _)| v);
            let skill = (skill.clamp(0.0, 1.0) * 1_000_000.0) as u32;
            Some((j.start_key(), std::cmp::Reverse(skill), s))
        })
        .min()
        .map(|(_, _, s)| s)
}

/// The Hire's and Layoff's extra actor: `b`'s foreman (careers on), `who`
/// left out.
pub fn foreman_actor(world: &World, b: EntityId, who: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    foreman(world, b, Some(who))
}

/// `economy::collect_wage`'s foreman multiplier (1 for anyone else).
pub fn foreman_mult(world: &World, job: &Job) -> f32 {
    if job.foreman {
        world.config.careers.foreman_premium
    } else {
        1.0
    }
}

/// Midnight: `Job.foreman` on each staffed building's foreman (a crew of two
/// or more), off for everyone else.
fn foremen(world: &mut World) {
    let mut employers: BTreeSet<EntityId> = BTreeSet::new();
    let mut flagged: Vec<EntityId> = Vec::new();
    for id in world.citizens() {
        if let Some(j) = world.comp::<Job>(id) {
            if let Some(e) = j.employer {
                employers.insert(e);
            }
            if j.foreman {
                flagged.push(id);
            }
        }
    }
    let mut chosen: BTreeSet<EntityId> = BTreeSet::new();
    for b in employers {
        if world.staff_of(b).len() < 2 {
            continue;
        }
        if let Some(f) = foreman(world, b, None) {
            chosen.insert(f);
        }
    }
    for id in flagged {
        if !chosen.contains(&id) {
            if let Some(j) = world.comp_mut::<Job>(id) {
                j.foreman = false;
            }
        }
    }
    for id in chosen {
        if let Some(j) = world.comp_mut::<Job>(id) {
            j.foreman = true;
        }
    }
}

// ---------------------------------------------------------------------------
// § 4.3 The profession
// ---------------------------------------------------------------------------

/// `id` holds `role` as its profession (careers on, `profession_min_days`
/// worked).
pub fn holds(world: &World, id: EntityId, role: Role) -> bool {
    on(world)
        && world
            .comp::<Profession>(id)
            .is_some_and(|p| p.trade == role && p.days >= world.config.careers.profession_min_days)
}

/// The world-side shift end (both tiers: `exec::actions::end_shift` and
/// `lod::stat_work`, beside `service::shift_end`): a day in the Job's role,
/// once a day. A role other than the profession counts its own run, which
/// replaces the profession at `profession_min_days`. Nothing off.
pub fn shift_end(world: &mut World, id: EntityId) {
    if !on(world) {
        return;
    }
    let Some(role) = world.comp::<Job>(id).map(|j| j.role) else { return };
    let day = world.day().min(u64::from(u16::MAX)) as u16;
    let min = world.config.careers.profession_min_days;
    let Some(p) = world.comp_mut::<Profession>(id) else {
        world.insert(id, Profession { trade: role, days: 1, last_day: day, next: None });
        return;
    };
    // Review fix 11: one day a day, day 0 included (a seeded profession's
    // `last_day` 0 counts its seeded tenure as day 0's).
    if p.last_day == day && p.days > 0 {
        return;
    }
    p.last_day = day;
    if p.trade == role {
        p.days += 1;
        p.next = None;
    } else if p.days < min {
        *p = Profession { trade: role, days: 1, last_day: day, next: None };
    } else {
        let n = match p.next {
            Some((r, d)) if r == role => d.saturating_add(1),
            _ => 1,
        };
        if u32::from(n) >= min {
            *p = Profession { trade: role, days: u32::from(n), last_day: day, next: None };
        } else {
            p.next = Some((role, n));
        }
    }
}

// ---------------------------------------------------------------------------
// § 4.4 Retirement and the frailty exit
// ---------------------------------------------------------------------------

/// The household's coins and its daily costs for `id`: `id`'s and a living
/// spouse's (at the same Home) coins; `mean price × 2 meals × (1 + spouse)
/// + the Home's rent a day`.
fn household_cover(world: &World, id: EntityId) -> (i64, i64) {
    let home = world.comp::<crate::components::Household>(id).and_then(|h| h.home);
    let spouse = world.spouse_of(id).filter(|&s| {
        crate::systems::law::living(world, s)
            && world.comp::<crate::components::Household>(s).and_then(|h| h.home) == home
    });
    let coins = world.purse(Some(id)) + spouse.map_or(0, |s| world.purse(Some(s)));
    let mouths = 1 + i64::from(spouse.is_some());
    let rent = home.map_or(0, |h| ownership::rent_for(world, h));
    (coins, world.mean_price().max(1) * 2 * mouths + rent)
}

/// Midnight (from `living::run`, after `ages::daily`): the retired under
/// `unretire_coins` back in the labour pool; the workers past
/// `retire_from_years` to kin or on savings; the frailty exits; the foremen.
pub fn daily(world: &mut World) {
    if on(world) {
        let _t = crate::prof::scope!("careers::daily");
        unretire(world);
        retire(world);
        frail_exits(world);
        foremen(world);
    }
    if lieutenants_on(world) {
        gang_day(world);
    }
}

fn unretire(world: &mut World) {
    let floor = world.config.careers.unretire_coins;
    // Under a grown child's roof the child keeps him: still retired.
    let back: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&a| world.has::<Retired>(a) && world.purse(Some(a)) < floor)
        .filter(|&a| !crate::systems::ages::with_kin(world, a))
        .collect();
    for a in back {
        world.remove::<Retired>(a);
        ownership::jobs2_row(world).unretired += 1;
    }
}

fn retire(world: &mut World) {
    let cfg = world.config.careers.clone();
    let kin_p = world.config.ages.kin_take_in_p;
    let ages_on = crate::systems::ages::on(world);
    let day = world.day();
    let workers: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&a| world.has::<Job>(a) && !world.has::<Sentence>(a) && years(world, a) >= cfg.retire_from_years)
        .collect();
    for a in workers {
        // The kin take-in first (the `[ages]` rule and odds), then savings.
        // Review fix 4: a co-resident spouse moves with him, so the child's
        // Home needs room for both; room for one only, no retirement to kin.
        let spouse = co_resident_spouse(world, a);
        let kin = ages_on
            .then(|| crate::systems::ages::kin_home(world, a))
            .flatten()
            .filter(|&h| spouse.is_none() || room_for(world, h, 2))
            .filter(|_| crate::systems::econ::hash_unit(world.seed(), PURPOSE_RETIRE_KIN, key(a), day) < kin_p);
        let (coins, cost) = household_cover(world, a);
        let saved = coins >= i64::from(cfg.retire_days) * cost;
        if kin.is_none() && !saved {
            continue;
        }
        let Some(job) = world.vacate_job(a) else { continue };
        world.abort_plan(a);
        crate::systems::life::note_quit(world, a, job.employer);
        let now = world.tick;
        world.insert(a, Retired { since: now });
        let name = world.name_of(a);
        let at = job.employer.map_or_else(String::new, |e| format!(" at {}", world.name_of(e)));
        let how = if kin.is_some() { "to family" } else { "on savings" };
        let mut actors = vec![a];
        actors.extend(job.employer);
        world.push_event(EventKind::Retired, &actors, format!("{name} retired from {}{at} ({how})", job.role.label()));
        ownership::jobs2_row(world).retirements += 1;
        if let Some(h) = kin {
            crate::systems::ages::move_in(world, a, h);
            if let Some(sp) = spouse {
                crate::systems::ages::move_in_with(world, sp, h);
            }
        }
    }
}

/// Review fix 4: `a`'s living spouse when the two share a Home.
fn co_resident_spouse(world: &World, a: EntityId) -> Option<EntityId> {
    let home = world.comp::<crate::components::Household>(a).and_then(|h| h.home)?;
    world.spouse_of(a).filter(|&s| {
        crate::systems::law::living(world, s)
            && world.comp::<crate::components::Household>(s).and_then(|h| h.home) == Some(home)
    })
}

/// Review fix 4: the Home `h` has room for `n` more residents.
fn room_for(world: &World, h: EntityId, n: usize) -> bool {
    world
        .comp::<crate::components::Building>(h)
        .is_some_and(|b| world.residents_of(h).len() + n <= usize::from(b.capacity.max(1)))
}

fn frail_exits(world: &mut World) {
    if !crate::systems::ages::on(world) {
        return;
    }
    let mult = world.config.careers.frail_exit_mult;
    let from = world.config.ages.frail_from_years;
    let frail: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&a| world.has::<Job>(a) && !world.has::<Sentence>(a))
        .filter(|&a| {
            let y = years(world, a);
            y >= from && crate::systems::ages::work_mult(world, a) < mult
        })
        .collect();
    for a in frail {
        let building = world.comp::<Job>(a).and_then(|j| j.employer);
        let name = world.name_of(a);
        crate::systems::economy::dismiss(world, a, building, format!("{name} was let go: too frail to work"));
        ownership::jobs2_row(world).frail_exits += 1;
    }
}

// ---------------------------------------------------------------------------
// § 4.5 Leaders lead
// ---------------------------------------------------------------------------

/// `id` leads a gang that has `delegate_min_fit` fit members (lieutenants
/// on): he neither marches nor shakes down.
pub fn delegating(world: &World, id: EntityId) -> bool {
    if !lieutenants_on(world) {
        return false;
    }
    let Some(gang) = world.gang_of(id) else { return false };
    world.comp::<Gang>(gang).is_some_and(|g| g.leader == Some(id))
        && crate::systems::gang::fit_headcount(world, gang) >= world.config.gangs.delegate_min_fit
}

/// The lieutenants' count for a gang of `members`: `1 + members / lieutenant_per`.
pub fn lieutenant_count(world: &World, members: usize) -> usize {
    1 + members / world.config.gangs.lieutenant_per.max(1)
}

/// The raid's head among `present` (the raiders at the door): the leader if
/// there, else the best-ranked lieutenant (`leader_ranking`), else `actor`.
pub fn raid_head(world: &World, gang: EntityId, actor: EntityId, present: &[EntityId]) -> EntityId {
    if !lieutenants_on(world) {
        return actor;
    }
    let rank = |m: EntityId| world.comp::<GangMember>(m).map_or(0, |g| g.rank);
    if let Some(&l) = present.iter().find(|&&m| rank(m) == 2) {
        return l;
    }
    crate::systems::gang::leader_ranking(world, gang)
        .into_iter()
        .find(|m| rank(*m) == 1 && present.contains(m))
        .unwrap_or(actor)
}

/// The ladder: the head led a raid (won: `won`); a lieutenant's counted.
pub fn credit_raid(world: &mut World, head: EntityId, won: bool) {
    if !lieutenants_on(world) {
        return;
    }
    let Some(gm) = world.comp_mut::<GangMember>(head) else { return };
    gm.raids_led = gm.raids_led.saturating_add(1);
    if won {
        gm.raids_won = gm.raids_won.saturating_add(1);
    }
    if gm.rank == 1 {
        ownership::jobs2_row(world).lieutenant_raids += 1;
    }
}

/// Midnight: the day's lieutenants and the leader-days delegated (a leader
/// delegating under an order he would otherwise work or march).
fn gang_day(world: &mut World) {
    use crate::components::Order;
    let (mut lts, mut delegated) = (0u32, 0u32);
    for gang in world.gangs() {
        let Some(g) = world.comp::<Gang>(gang) else { continue };
        lts += g.members.iter().filter(|&&m| world.comp::<GangMember>(m).is_some_and(|x| x.rank == 1)).count() as u32;
        let worked = g.order.is_raid()
            || matches!(g.order, Order::Expand | Order::Squat | Order::Harvest | Order::VirtRaid | Order::Job);
        if worked && g.leader.is_some_and(|l| delegating(world, l)) {
            delegated += 1;
        }
    }
    if lts > 0 || delegated > 0 {
        let row = ownership::jobs2_row(world);
        row.lieutenants += lts;
        row.delegated_orders += delegated;
    }
}
