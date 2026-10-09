//! Jobs and room P3 (docs/JOBS_V2.md § 1.2-1.3, plan J9-J15): the
//! configured trades' places, their posting where no other pass posts them,
//! their output and the Super's round. A trade is a `Role::Trade(TradeId)`
//! into `Config::trades`; its staff work the generic `Work` at the
//! workplace and are paid by the workplace's owner at the shift's end like
//! any staff (`economy::collect_wage`), so every wage comes out of the
//! owner's purse, which its building's revenue fills: no new money.

use crate::components::{ActionInstance, Brain, Building, BuildingKind, Corp, GoalKind, Job, Position, Role, Sentence};
use crate::config::{TradeCfg, TradeDuty, TradeModel};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::ownership::{self, OwnerKind};
use crate::world::World;

/// A trade's row (`None` for a bespoke role or a row the config lacks).
pub fn row_of(world: &World, role: Role) -> Option<&TradeCfg> {
    role.trade().and_then(|t| world.config.trade(t))
}

/// `id`'s trade row, if it holds a trade's Job.
fn job_row(world: &World, id: EntityId) -> Option<&TradeCfg> {
    world.comp::<Job>(id).and_then(|j| row_of(world, j.role))
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

// ---------------------------------------------------------------------------
// Places (J10, J14)
// ---------------------------------------------------------------------------

/// J14: a `per_owned` trade's places at `b` (the Super), out of the corp's
/// `heads = ceil(standing buildings of the trade's kinds ÷ per_owned)`
/// posts (`staff_per_floor` each). Review fix (stable posts): a building
/// already employing the trade keeps its post (the lowest-id `heads` of
/// them, if the corp shrank below its staff); the heads left unserved go to
/// the unserved group anchors (the lowest id of each group of `per_owned`,
/// `Corp.buildings` ascending), then to the other unserved buildings, in id
/// order. Selling or losing a lower-id Block no longer moves every anchor
/// (a layoff at each old post and a hire at each new one). 0 for any other
/// building, a non-corp owner or a building that does not stand.
pub fn owned_places(world: &World, b: EntityId, row: &TradeCfg) -> usize {
    let Some(c) = world.owner_of(b).and_then(|o| world.comp::<Corp>(o)) else { return 0 };
    let n = usize::from(row.per_owned.max(1));
    let held = |x: EntityId| {
        world
            .staff_of(x)
            .iter()
            .any(|&s| world.comp::<Job>(s).is_some_and(|j| row_of(world, j.role).is_some_and(|r| r.id == row.id)))
    };
    let (mut served, mut anchors, mut others) = (Vec::new(), Vec::new(), Vec::new());
    let mut k = 0usize;
    for &x in &c.buildings {
        let Some(bd) = world.comp::<Building>(x) else { continue };
        if bd.demolished || bd.derelict || !row.workplace.contains(&bd.kind) {
            continue;
        }
        if held(x) {
            served.push(x);
        } else if k.is_multiple_of(n) {
            anchors.push(x);
        } else {
            others.push(x);
        }
        k += 1;
    }
    let heads = k.div_ceil(n);
    let posts = served.iter().chain(&anchors).chain(&others).take(heads);
    if posts.into_iter().any(|&x| x == b) {
        usize::from(row.staff_per_floor)
    } else {
        0
    }
}

/// The kinds some trade works at that `jobs::top_up`'s own lists (Markets,
/// Bars, the leisure venues, Fabs) do not walk, ascending.
fn trade_only_kinds(world: &World) -> Vec<BuildingKind> {
    let mut kinds: Vec<BuildingKind> = world
        .config
        .trades
        .iter()
        .flat_map(|r| r.workplace.iter().copied())
        .filter(|&k| !matches!(k, BuildingKind::Market | BuildingKind::Bar | BuildingKind::Fab) && !k.is_leisure())
        .collect();
    kinds.sort();
    kinds.dedup();
    kinds
}

/// J15, daily (from `jobs::top_up`) and at seed: the trade places of every
/// standing building of a kind [`trade_only_kinds`] lists are posted to
/// their deficit (the staff in the role and the open vacancies for it),
/// a hunkering corp's to half. A corp's buildings with wages on are the
/// margin rule's (`wages::post` walks every role of a kind); an agent's or a
/// gang's post one a day while the purse holds the wage (the venues' rule);
/// the city's and a corp's post the whole deficit. With wages off a corp's
/// bespoke role there is topped up too: `corp_brain::staff_up` counts every
/// staff and vacancy against the bespoke role's places, so a building with
/// trade staff would otherwise stay short of it (J10's recorded gap until
/// P4 edits `corp_brain`).
pub fn top_up(world: &mut World) {
    if world.config.trades.is_empty() {
        return;
    }
    let wages = crate::systems::wages::on(world);
    for kind in trade_only_kinds(world) {
        let roles = ownership::roles_for(world, kind);
        for b in world.buildings_of_kind(kind).to_vec() {
            if !standing(world, b) {
                continue;
            }
            let owner = world.owner_of(b);
            let kind_of_owner = ownership::owner_kind(world, owner);
            let corp = matches!(kind_of_owner, OwnerKind::Corp(_));
            if wages && corp {
                continue;
            }
            for &(role, _) in &roles {
                if !role.is_trade() && !(corp && !wages) {
                    continue;
                }
                let full = crate::systems::jobs::places_of(world, b, role);
                let short = crate::systems::jobs::deficit(world, b, role, full);
                if short == 0 {
                    continue;
                }
                let n = match kind_of_owner {
                    OwnerKind::City | OwnerKind::Corp(_) => short,
                    _ => usize::from(world.purse(owner) >= world.config.wage(role)),
                };
                if n > 0 {
                    world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, n));
                }
            }
        }
    }
}

/// J15 (`World::new`, after every seeded building stands): the trade
/// places of the buildings the map and the seeding passes put up without
/// `founding::convert` (Markets, Clinics, the corps' Blocks), posted as
/// full staff, every kind a trade works at, wages or not. No RNG.
pub fn seed(world: &mut World) {
    if world.config.trades.is_empty() {
        return;
    }
    let mut kinds: Vec<BuildingKind> = world.config.trades.iter().flat_map(|r| r.workplace.iter().copied()).collect();
    kinds.sort();
    kinds.dedup();
    for kind in kinds {
        let roles: Vec<Role> =
            ownership::roles_for(world, kind).into_iter().map(|(r, _)| r).filter(|r| r.is_trade()).collect();
        for b in world.buildings_of_kind(kind).to_vec() {
            if !standing(world, b) {
                continue;
            }
            for &role in &roles {
                let full = crate::systems::jobs::places_of(world, b, role);
                let short = crate::systems::jobs::deficit(world, b, role, full);
                if short > 0 {
                    world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, short));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The shift (J12, J13)
// ---------------------------------------------------------------------------

/// J12: a trade's shifts for `id`: `day` the `[world] shift_day`, `evening`
/// the `[leisure] evening_shift`, `night` the `[world] shift_night`, `split`
/// night for an even id and day for an odd one (the guards' rule).
pub fn shifts_for(world: &World, row: &TradeCfg, id: EntityId) -> Vec<(u16, u16)> {
    use crate::config::TradeShift;
    let wc = &world.config.world;
    match row.shift {
        TradeShift::Day => wc.shift_day.clone(),
        TradeShift::Evening => world.config.leisure.evening_shift.clone(),
        TradeShift::Night => wc.shift_night.clone(),
        TradeShift::Split if id.index.is_multiple_of(2) => wc.shift_night.clone(),
        TradeShift::Split => wc.shift_day.clone(),
    }
}

/// J13 (`produce`): a producing trade's `ticks` on shift at `b` make the
/// building's good as its kind's own staff do (the Farm's and the Fab's
/// accrual, skill included); a kind with no output yet makes nothing
/// (P4's Packing Plant adds its own). A no-op for any other model.
pub fn accrue(world: &mut World, worker: EntityId, b: EntityId, ticks: u64) {
    if job_row(world, worker).is_none_or(|r| r.model != TradeModel::Produce) {
        return;
    }
    match world.comp::<Building>(b).map(|bd| bd.kind) {
        Some(BuildingKind::Farm) => crate::systems::economy::accrue_farm_work(world, worker, b, ticks),
        Some(BuildingKind::Fab) => crate::systems::jobs::accrue_fab_work(world, worker, b, ticks),
        _ => {}
    }
}

/// J13 (`service`): `b`'s `service` trade staff on shift now, inside it (a
/// Statistical one counts as there), not held: the building's on-duty
/// staff where its service reads them (a Club's door, `leisure::door_ok`).
pub fn service_on_duty(world: &World, b: EntityId) -> Vec<EntityId> {
    let tod = world.tick_of_day();
    world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| {
            world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod) && row_of(world, j.role).is_some_and(|r| r.model == TradeModel::Service)
            }) && !world.has::<Sentence>(s)
                && world.comp::<Brain>(s).is_some_and(|br| {
                    br.lod == crate::components::Lod::Statistical
                        || world.comp::<Position>(s).is_some_and(|p| p.building == Some(b))
                })
        })
        .collect()
}

/// J13 (`service`): the on-duty service staff who keep `b`'s door: the
/// fighting trades (the Bouncers), for `leisure::door_ok`.
pub fn door_staff(world: &World, b: EntityId) -> Vec<EntityId> {
    let mut out = service_on_duty(world, b);
    out.retain(|&s| job_row(world, s).is_some_and(|r| r.skill == crate::config::TradeSkill::Fighting));
    out
}

/// J13: a trade's staff are let into their own workplace when it is full
/// (a Night Porter into a booked-out Hotel, a Bouncer into a packed Club),
/// and a Super into the Blocks of his round.
pub fn capacity_exempt(world: &World, agent: EntityId, b: EntityId) -> bool {
    let Some(j) = world.comp::<Job>(agent).filter(|j| j.role.is_trade()) else { return false };
    if j.employer == Some(b) {
        return true;
    }
    job_row(world, agent).is_some_and(|r| r.duty == TradeDuty::Round)
        && world.comp::<Brain>(agent).is_some_and(|br| br.patrol_route.contains(&b))
}

/// J13: is `agent` a trade's worker inside its own workplace (the goal
/// state's `Workplace`, as a Volunteer inside its Mission)?
pub fn at_post(world: &World, agent: EntityId, b: EntityId) -> bool {
    world.comp::<Job>(agent).is_some_and(|j| j.role.is_trade() && j.employer == Some(b))
}

// ---------------------------------------------------------------------------
// The Super's round (J14)
// ---------------------------------------------------------------------------

/// J14: the round for a shift: `round_stops` of the employer corp's
/// standing Blocks in the post's district (the post not counted), taken
/// from the id-ordered list at `shift_key × round_stops` (so the round
/// turns through all of them over the days), walked nearest-first from
/// the post's door.
fn new_round(world: &World, post: EntityId, row: &TradeCfg, shift_key: i64) -> Vec<EntityId> {
    let Some(c) = world.owner_of(post).and_then(|o| world.comp::<Corp>(o)) else { return Vec::new() };
    let d = world.district_of_building(post);
    let blocks: Vec<EntityId> = c
        .buildings
        .iter()
        .copied()
        .filter(|&x| x != post && standing(world, x))
        .filter(|&x| world.comp::<Building>(x).is_some_and(|bd| row.workplace.contains(&bd.kind)))
        .filter(|&x| world.district_of_building(x) == d)
        .collect();
    let stops = usize::from(row.round_stops).min(blocks.len());
    if stops == 0 {
        return Vec::new();
    }
    let start = (shift_key.max(0) as usize).wrapping_mul(stops) % blocks.len();
    let mut left: Vec<EntityId> = (0..stops).map(|i| blocks[(start + i) % blocks.len()]).collect();
    let door = |x: EntityId| world.comp::<Building>(x).map(|bd| bd.door).unwrap_or_default();
    let mut from = door(post);
    let mut route = Vec::with_capacity(stops);
    while !left.is_empty() {
        let (i, _) = left.iter().enumerate().min_by_key(|&(_, &x)| (door(x).manhattan(from), x)).expect("non-empty");
        let x = left.remove(i);
        from = door(x);
        route.push(x);
    }
    route
}

/// J14 review: the end of a Super's shift kept for the walk back to the
/// post and the shift's close there (two hours: a district's round).
pub const ROUND_CUTOFF: u64 = 2 * crate::time::TICKS_PER_HOUR;

/// J14, the Work goal's bypass (`plan::plan_for`, after the commute): a
/// Super on his shift walks his round one door at a time: each call marks
/// the stop he stands in (`Brain.patrol_legs`, the guards' patrol fields,
/// fresh per shift) and plans the walk to the next one; with the round
/// walked (`SuperRound`) it returns `None` and the planner sends him to
/// his post for the rest of the shift, where the wage is paid at its end.
pub fn round_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // The cheap test first, on borrows: most callers hold no round trade.
    {
        let j = world.comp::<Job>(id)?;
        row_of(world, j.role).filter(|r| r.duty == TradeDuty::Round)?;
        if j.employer.is_none() || !j.on_shift(world.tick_of_day()) {
            return None;
        }
        // Review fix: the last `ROUND_CUTOFF` of the shift belongs to the
        // post (where the shift is worked and paid at its end): a round
        // still walking at 18:00 lost the day's wage.
        let end = j.shift_end(world.tick)?;
        if end.saturating_sub(world.tick) <= ROUND_CUTOFF {
            return None;
        }
    }
    let job = world.comp::<Job>(id)?.clone();
    let row = row_of(world, job.role)?.clone();
    let post = job.employer?;
    let key = job.next_shift_key(world.tick);
    if job.last_shift_day == Some(key) || !crate::exec::routine::workday_of(world, id, &job, key) {
        return None;
    }
    let fresh = world.comp::<Brain>(id).is_none_or(|b| b.patrol_shift_key != Some(key));
    if fresh {
        let route = new_round(world, post, &row, key);
        let b = world.comp_mut::<Brain>(id)?;
        b.patrol_route = route;
        b.patrol_legs = 0;
        b.patrol_shift_key = Some(key);
    }
    let here = world.comp::<Position>(id).and_then(|p| p.building);
    let (route, mut legs) = world.comp::<Brain>(id).map(|b| (b.patrol_route.clone(), usize::from(b.patrol_legs)))?;
    if legs >= route.len() {
        return None;
    }
    if here == Some(route[legs]) {
        legs += 1;
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.patrol_legs = u8::try_from(legs).unwrap_or(u8::MAX);
        }
        if legs >= route.len() {
            let (name, what) = (world.name_of(id), world.owner_label(world.owner_of(post)));
            let at = world.district_name(world.district_of_building(post)).to_string();
            let text = format!("{name} walked the round of {} {what} Blocks in {at}", route.len());
            world.push_event(EventKind::SuperRound, &[id, post], text);
            world.stats.current.jobs.super_rounds += 1;
            return None;
        }
    }
    let next = route[legs];
    let step = ActionInstance { action: ActionKind::GoTo(LocationKey::TargetHome), target: Some(next), tile: None };
    Some(Plan { goal: GoalKind::Work, target: Some(next), steps: vec![step], started_tick: world.tick })
}
