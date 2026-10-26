//! The pre-view pass phase 6 (docs/PREVIEW_PASS.md § 0 P9, P13-P15;
//! OPEN-118, OPEN-128): **the leader's visible half**, under `[leaders]`
//! (`None`: every entry point the old path).
//! It applies to a leader with a body (Full or Coarse, free, alive).
//!
//! **The round** (P13): from `round_hour` the leader's Lead plan walks the
//! gang's businesses, nearest-first: the protected businesses owing a week
//! first, then the gang's fronts, `round_stops_max` in all. At each stop
//! (`Collect`, `round_stop`) every owed week of the gang's protection on it
//! is collected **in person** from whoever answers for it (the owner, a
//! corp's exec, a rival's leader) if he is there: his body inside, awake, or
//! an off-screen owner on a purpose-keyed draw (`owner_stat_p`). Out, the
//! stop collects nothing that day and the week stays owed. The midnight
//! collection (`leverage::collect`) no longer pays a body leader's gang; it
//! only ends a hold that is gone, and a new protection's first week waits
//! for the round. P16 (Dylan 2026-10-21): whoever is on shift at the business
//! and present pays the week from the till (`pay_week`'s purse, the owner's);
//! a Statistical worker on shift is present; the owner answers only when
//! nobody is on shift. The coins and their flow are `pay_week`'s, so the ledger
//! holds (P5); what moves is when and whether a week is met (a FINDING).
//!
//! **Court** (P14): from `court_hour` the leader walks to the Hideout and
//! holds court (`ActionKind::Court`) for `court_min`. A lieutenant there
//! (his body inside, or an off-screen one drawn at `court_stat_p` per court)
//! takes a waiting order; a member's `SplitLoot` at the Hideout during court
//! is handed to the leader in person (`TakeHanded`; the coins reach the
//! treasury as before). No petitioners (the pressure pass).
//!
//! **Orders in person** (P15): with a body leader and a lieutenant, an order
//! the faction brain chooses is held (`Gang.pending_order`) and the members
//! keep the old one until it is given: at court; past `order_wait_ticks`
//! the leader walks to a lieutenant and gives it where he stands (an
//! off-screen one at his Home, met on the presence draw); past
//! `order_word_ticks` more it goes by word (counted `orders_word`). A raid's
//! muster is timed from the giving (`faction::apply_order`): a raid may
//! slip, it never vanishes.
//!
//! **Statistical mirror** (doctrine 7): a Statistical leader keeps the
//! off-screen path, unchanged: the protection paid at midnight and the order
//! in force the moment it is chosen. He is off camera by definition
//! (OPEN-149) and has no body to walk a round or hold court; a draw would
//! only add a stream for a scene nobody sees. A gang whose leader turns
//! Statistical (or loses his last lieutenant, or goes to jail) has its
//! waiting order put in force at once (`run`).

use crate::components::{ActionInstance, Brain, Building, Gang, GangMember, GoalKind, Household, PendingOrder};
use crate::components::{Position, Sentence};
use crate::config::LeadersCfg;
use crate::contract::{CoercionId, CoercionWhy, Demand};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::ExecState;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::{law, ownership, presence};
use crate::time::{Tick, TICKS_PER_HOUR};
use crate::world::World;

/// The draw purposes (`econ::hash_unit`, unique to their sites).
pub const PURPOSE_OWNER: u64 = 0x4C44_0001;
pub const PURPOSE_COURT: u64 = 0x4C44_0002;

/// `[leaders]` is on.
pub fn on(world: &World) -> bool {
    world.config.leaders.is_some()
}

fn cfg(world: &World) -> Option<&LeadersCfg> {
    world.config.leaders.as_ref()
}

fn free(world: &World, id: EntityId) -> bool {
    law::living(world, id) && !world.has::<Sentence>(id)
}

/// The gang's leader when he has a body (Full or Coarse), free and alive.
pub fn body_leader(world: &World, gang: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let l = world.comp::<Gang>(gang)?.leader?;
    (presence::is_body(world, l) && free(world, l)).then_some(l)
}

/// P13: the gang's protection is collected on its leader's round (not at midnight).
pub fn collects(world: &World, gang: EntityId) -> bool {
    body_leader(world, gang).is_some()
}

/// The gang's free lieutenants (rank 1), in roster order.
pub fn lieutenants(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    g.members
        .iter()
        .copied()
        .filter(|&m| Some(m) != g.leader)
        .filter(|&m| world.comp::<GangMember>(m).is_some_and(|x| x.rank == 1) && free(world, m))
        .collect()
}

/// P15: an order this gang chooses waits to be given to a lieutenant in person.
pub fn in_person(world: &World, gang: EntityId) -> bool {
    body_leader(world, gang).is_some() && !lieutenants(world, gang).is_empty()
}

/// P15: hold a chosen order for the giving. The same order chosen again
/// keeps its first `decided` (the wait is not reset); another replaces it.
pub fn hold(world: &mut World, gang: EntityId, p: PendingOrder) {
    let Some(g) = world.comp_mut::<Gang>(gang) else { return };
    match g.pending_order.as_mut() {
        Some(old) if old.order == p.order => {
            old.corp_target = p.corp_target;
            old.retaliate_on = p.retaliate_on;
            old.why = p.why;
        }
        _ => g.pending_order = Some(p),
    }
}

fn tod(world: &World) -> u64 {
    u64::from(world.tick_of_day())
}

/// Court's window today, in ticks of the day.
fn court_window(c: &LeadersCfg) -> (u64, u64) {
    let s = u64::from(c.court_hour) * TICKS_PER_HOUR;
    (s, s + u64::from(c.court_min))
}

fn door(world: &World, b: EntityId) -> Option<crate::components::TilePos> {
    world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| bd.door)
}

/// The gang's protection weeks owed now: `(coercion, business)`.
fn owed(world: &World, gang: EntityId) -> Vec<(CoercionId, EntityId)> {
    let now = world.tick;
    world
        .coercions
        .values()
        .filter(|c| c.is_active() && c.why == CoercionWhy::Protection && c.holder == Some(gang))
        .filter(|c| c.next_due <= now && matches!(c.demand, Demand::Coins { weekly: true, .. }))
        .filter_map(|c| c.building.map(|b| (c.id, b)))
        .collect()
}

/// P13: the round's stops from the leader's tile: the businesses owing a
/// week first, then the fronts (each by distance), `round_stops_max` of
/// them, walked nearest-first.
pub fn round_stops(world: &World, gang: EntityId, leader: EntityId) -> Vec<EntityId> {
    let Some(c) = cfg(world) else { return Vec::new() };
    let Some(from) = world.comp::<Position>(leader).map(|p| p.tile) else { return Vec::new() };
    let key = |b: EntityId| door(world, b).map(|d| (d.manhattan(from), b));
    let mut owing: Vec<(u32, EntityId)> = owed(world, gang).into_iter().filter_map(|(_, b)| key(b)).collect();
    owing.sort_unstable();
    owing.dedup_by_key(|x| x.1);
    let mut fronts: Vec<(u32, EntityId)> = crate::systems::leisure::fronts_of(world, gang)
        .into_iter()
        .filter(|b| !owing.iter().any(|x| x.1 == *b))
        .filter_map(key)
        .collect();
    fronts.sort_unstable();
    let mut pick: Vec<EntityId> =
        owing.into_iter().chain(fronts).map(|x| x.1).take(usize::from(c.round_stops_max)).collect();
    let mut out = Vec::with_capacity(pick.len());
    let mut at = from;
    while !pick.is_empty() {
        let i = (0..pick.len())
            .min_by_key(|&i| (door(world, pick[i]).map_or(u32::MAX, |d| d.manhattan(at)), pick[i]))
            .unwrap_or(0);
        let b = pick.remove(i);
        at = door(world, b).unwrap_or(at);
        out.push(b);
    }
    out
}

fn round_due(world: &World, gang: EntityId, leader: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    let t = tod(world);
    let open = t >= u64::from(c.round_hour) * TICKS_PER_HOUR && t < u64::from(c.round_last_hour) * TICKS_PER_HOUR;
    open && world.comp::<Gang>(gang).is_some_and(|g| g.round_day != Some(world.day()))
        && !round_stops(world, gang, leader).is_empty()
}

/// Court is due: today's not held, and the leader sets out in time to be
/// at the Hideout by `court_hour` (his trip's estimate, at most
/// `court_walk_max` ahead), arriving before its end.
fn court_due(world: &World, gang: EntityId, leader: EntityId) -> bool {
    world.comp::<Gang>(gang).is_some_and(|g| g.court_day != Some(world.day())) && sets_out(world, gang, leader)
}

/// `who` sets out for court now: his trip to the Hideout (at most
/// `court_walk_max`) lands him there from `court_hour` and before its end.
fn sets_out(world: &World, gang: EntityId, who: EntityId) -> bool {
    let Some(c) = cfg(world) else { return false };
    let (s, e) = court_window(c);
    let t = tod(world);
    if t >= e || t + c.court_walk_max < s {
        return false;
    }
    let Some(door) = world.hideout_of(gang).and_then(|h| door(world, h)) else { return false };
    let walk = crate::systems::fixes::walk_estimate(world, who, door).min(c.court_walk_max);
    t + walk >= s && t + walk < e
}

/// P14: a lieutenant with a body is sent for to court while an order waits
/// for him (his gang's leader a body).
pub fn attend_due(world: &World, id: EntityId) -> bool {
    if !on(world) || !presence::is_body(world, id) {
        return false;
    }
    let Some(gang) = world.gang_of(id) else { return false };
    world.comp::<Gang>(gang).is_some_and(|g| g.pending_order.is_some())
        && body_leader(world, gang).is_some_and(|l| l != id)
        && lieutenants(world, gang).contains(&id)
        && sets_out(world, gang, id)
}

/// P14: the lieutenant's walk to court and his place at it to its end.
pub fn attend_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    let hideout = world.gang_of(id).and_then(|g| world.hideout_of(g))?;
    let steps =
        vec![step(ActionKind::GoTo(LocationKey::Seller), Some(hideout)), step(ActionKind::Court, Some(hideout))];
    Some(Plan { goal: GoalKind::Lead, target: Some(hideout), steps, started_tick: world.tick })
}

/// P15: the lieutenant a waiting order is walked to (past `order_wait_ticks`):
/// the nearest with a body, else the first off-screen one with a Home.
fn deliver_to(world: &World, gang: EntityId, leader: EntityId) -> Option<EntityId> {
    let c = cfg(world)?;
    let decided = world.comp::<Gang>(gang)?.pending_order.as_ref()?.decided;
    if world.tick < decided + c.order_wait_ticks {
        return None;
    }
    let from = world.comp::<Position>(leader)?.tile;
    let lts = lieutenants(world, gang);
    lts.iter()
        .copied()
        .filter(|&l| presence::is_body(world, l))
        .filter_map(|l| world.comp::<Position>(l).map(|p| (p.tile.manhattan(from), l)))
        .min()
        .map(|(_, l)| l)
        .or_else(|| lts.iter().copied().find(|&l| world.comp::<Household>(l).and_then(|h| h.home).is_some()))
}

/// The leader's Lead is due for the round, court or an order to walk.
pub fn due(world: &World, gang: EntityId, leader: EntityId) -> bool {
    body_leader(world, gang) == Some(leader)
        && (court_due(world, gang, leader)
            || deliver_to(world, gang, leader).is_some()
            || round_due(world, gang, leader))
}

fn step(action: ActionKind, target: Option<EntityId>) -> ActionInstance {
    ActionInstance { action, target, tile: None }
}

/// The Lead plan for what is due, in this order: court (the Collect and
/// Call before it when due), an order walked to a lieutenant, the round
/// (the Hideout's Collect after it when due). `None`: nothing of this
/// module's is due (the old Lead plan runs).
pub fn plan(world: &mut World, id: EntityId, gang: EntityId, hideout: EntityId) -> Option<Plan> {
    if body_leader(world, gang) != Some(id) {
        return None;
    }
    let today = world.day();
    let hideout_due =
        crate::systems::leisure::collect_due(world, gang) || crate::systems::leisure::call_due(world, gang);
    let mut steps = Vec::new();
    if court_due(world, gang, id) {
        steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(hideout)));
        if hideout_due {
            steps.push(step(ActionKind::Collect, Some(hideout)));
        }
        steps.push(step(ActionKind::Court, Some(hideout)));
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.court_day = Some(today);
        }
    } else if let Some(lt) = deliver_to(world, gang, id) {
        if presence::is_body(world, lt) {
            steps.push(step(ActionKind::GoTo(LocationKey::Victim), Some(lt)));
        } else {
            let home = world.comp::<Household>(lt).and_then(|h| h.home)?;
            steps.push(step(ActionKind::GoTo(LocationKey::TargetHome), Some(home)));
        }
        steps.push(step(ActionKind::Court, Some(lt)));
    } else if round_due(world, gang, id) {
        for b in round_stops(world, gang, id) {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(b)));
            steps.push(step(ActionKind::Collect, Some(b)));
        }
        if hideout_due {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(hideout)));
            steps.push(step(ActionKind::Collect, Some(hideout)));
        }
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.round_day = Some(today);
        }
        ownership::jobs2_row(world).lead_rounds += 1;
    } else {
        return None;
    }
    Some(Plan { goal: GoalKind::Lead, target: Some(hideout), steps, started_tick: world.tick })
}

/// Is an off-screen owner met at his business `b` today (P13's draw)?
fn owner_drawn(world: &World, over: EntityId, b: EntityId) -> bool {
    let p = cfg(world).map_or(0.0, |c| c.owner_stat_p);
    free(world, over)
        && crate::systems::econ::hash_unit(
            world.seed(),
            PURPOSE_OWNER,
            u64::from(over.index) ^ (u64::from(b.index) << 20),
            world.day(),
        ) < p
}

/// P16: who is on shift at `b` (living, free, not the leader), by id, with
/// whether he is present now: a body inside and awake, or a Statistical
/// worker (being on shift is the presence). `None`: nobody is on shift.
fn staff_on_shift(world: &World, b: EntityId, leader: EntityId) -> Option<Vec<(EntityId, bool)>> {
    let tod = world.tick_of_day();
    let staff: Vec<(EntityId, bool)> = world
        .staff_of(b)
        .iter()
        .copied()
        .filter(|&s| s != leader && free(world, s))
        .filter(|&s| {
            world.comp::<crate::components::Job>(s).is_some_and(|j| {
                j.on_shift(tod) && crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick))
            })
        })
        .map(|s| (s, !presence::is_body(world, s) || presence::body_at(world, s, b)))
        .collect();
    (!staff.is_empty()).then_some(staff)
}

/// P13, P16: the leader's `Collect` at business `b` on his round: every owed
/// week of the gang's protection on it, paid from the till by whoever is on
/// shift there and present; only when nobody is on shift does it fall to
/// whoever answers for the business (the owner, a corp's exec) if he is
/// there. Nobody there: nothing today, the week stays owed.
pub fn round_stop(world: &mut World, leader: EntityId, gang: EntityId, b: EntityId) {
    if body_leader(world, gang) != Some(leader) {
        return;
    }
    ownership::jobs2_row(world).lead_stops += 1;
    let staff = staff_on_shift(world, b, leader);
    let due: Vec<CoercionId> = owed(world, gang).into_iter().filter(|&(_, x)| x == b).map(|(id, _)| id).collect();
    for id in due {
        let Some(over) = world.coercions.get(&id).map(|c| c.over) else { continue };
        let clerk = staff.as_ref().and_then(|v| v.iter().find(|x| x.1).map(|x| x.0));
        let met = match &staff {
            Some(v) => v.iter().any(|x| x.1),
            None if presence::is_body(world, over) => presence::body_at(world, over, b),
            None => owner_drawn(world, over, b),
        };
        if !met {
            ownership::jobs2_row(world).lead_owner_out += 1;
            // The pressure pass phase 3 (OPEN-160): a stop that met nobody is
            // a missed week; at `punish_misses` the gang punishes.
            crate::systems::protection::miss(world, id);
            continue;
        }
        let paid = crate::systems::leverage::collect_in_person(world, id);
        if paid > 0 {
            ownership::jobs2_row(world).lead_collected += 1;
            let text = match clerk {
                Some(c) => format!(
                    "{} collected {paid} coins of protection at {} from the till, {} on shift",
                    world.name_of(leader),
                    world.name_of(b),
                    world.name_of(c)
                ),
                None => format!(
                    "{} collected {paid} coins of protection from {} at {}",
                    world.name_of(leader),
                    world.name_of(over),
                    world.name_of(b)
                ),
            };
            world.push_event(EventKind::Protected, &[leader, over, b], text);
        }
    }
}

/// The step target of `id`'s current step.
fn step_target(world: &World, id: EntityId) -> Option<EntityId> {
    world.comp::<Brain>(id).and_then(|b| b.current_step()).and_then(|s| s.target)
}

/// Court can start at the Hideout: from `court_walk_max` before it to as
/// long after its end (a walk that ran late still holds a short court).
fn court_open(world: &World, c: &LeadersCfg) -> bool {
    let (s, e) = court_window(c);
    let t = tod(world);
    t + c.court_walk_max >= s && t < e + c.court_walk_max
}

/// `Court`'s length: to court's end at the Hideout, `order_give_min` at a lieutenant.
pub fn court_ticks(world: &World, id: EntityId) -> Tick {
    let Some(c) = cfg(world) else { return 1 };
    let target = step_target(world, id);
    if target.is_some() && target == world.gang_of(id).and_then(|g| world.hideout_of(g)) {
        // A late arrival holds a short court (`order_give_min`).
        let (_, e) = court_window(c);
        return e.saturating_sub(tod(world)).max(u64::from(c.order_give_min).max(1));
    }
    u64::from(c.order_give_min).max(1)
}

/// `Court` can start: at the Hideout inside it in court hours; at a
/// lieutenant beside him (his building, or within `order_reach`) and him
/// awake, or an off-screen one met at his Home on the presence draw.
pub fn can_court(world: &World, id: EntityId, target: Option<EntityId>) -> bool {
    let (Some(c), Some(t), Some(gang)) = (cfg(world), target, world.gang_of(id)) else { return false };
    let Some(here) = world.comp::<Position>(id) else { return false };
    // P14: a lieutenant sent for takes his place at court.
    if body_leader(world, gang) != Some(id) {
        return Some(t) == world.hideout_of(gang)
            && here.building == Some(t)
            && court_open(world, c)
            && lieutenants(world, gang).contains(&id);
    }
    if Some(t) == world.hideout_of(gang) {
        return here.building == Some(t) && court_open(world, c);
    }
    if !lieutenants(world, gang).contains(&t) {
        return false;
    }
    if presence::is_body(world, t) {
        let Some(there) = world.comp::<Position>(t) else { return false };
        let beside = match (here.building, there.building) {
            (Some(a), Some(b)) => a == b,
            (None, None) => law::chebyshev(here.tile, there.tile) <= c.order_reach,
            _ => false,
        };
        return beside && !presence::asleep(world, t);
    }
    let home = world.comp::<Household>(t).and_then(|h| h.home);
    home.is_some() && here.building == home && presence::met_at_home(world, t, home.unwrap_or(t), u64::from(gang.index))
}

/// `Court` starts at a lieutenant: the waiting order is given there.
pub fn court_start(world: &mut World, id: EntityId, target: Option<EntityId>) {
    let Some(gang) = world.gang_of(id) else { return };
    if target.is_none() || target == world.hideout_of(gang) || body_leader(world, gang) != Some(id) {
        return;
    }
    if let Some(lt) = target {
        give(world, gang, Some(id), Some(lt), Given::Walked);
    }
}

/// `Court` ends: a court held at the Hideout is counted.
pub fn court_done(world: &mut World, id: EntityId, target: Option<EntityId>) {
    let Some(gang) = world.gang_of(id) else { return };
    if target.is_some() && target == world.hideout_of(gang) && body_leader(world, gang) == Some(id) {
        ownership::jobs2_row(world).courts += 1;
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Given {
    Court,
    Walked,
    Word,
}

/// Put the waiting order in force, given by `leader` to `lt` (`how`).
fn give(world: &mut World, gang: EntityId, leader: Option<EntityId>, lt: Option<EntityId>, how: Given) {
    let Some(p) = world.comp::<Gang>(gang).and_then(|g| g.pending_order.clone()) else { return };
    let order = p.order;
    let lname = lt.map_or_else(String::new, |l| world.name_of(l));
    let note = match how {
        Given::Court => format!("given to {lname} at court"),
        Given::Walked => format!("given to {lname} where he was"),
        Given::Word => "sent by word".to_string(),
    };
    crate::systems::faction::apply_order(world, gang, p, Some(&note));
    {
        let row = ownership::jobs2_row(world);
        match how {
            Given::Court => row.orders_court += 1,
            Given::Walked => row.orders_walked += 1,
            Given::Word => row.orders_word += 1,
        }
    }
    if how == Given::Court {
        // The gang heard it at court: no separate Call for it.
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            g.called_for = Some(g.order_since);
        }
    }
    if let (Some(l), Some(t)) = (leader, lt) {
        let at = if how == Given::Court { "at court" } else { "in person" };
        let text = format!("{} gave {} the order {at}: {order:?}", world.name_of(l), world.name_of(t));
        world.push_event(EventKind::OrderGiven, &[l, t, gang], text);
    }
}

/// Is the gang's leader holding court at the Hideout now?
fn holding_court(world: &World, gang: EntityId, leader: EntityId) -> bool {
    let hideout = world.hideout_of(gang);
    hideout.is_some()
        && world.comp::<Position>(leader).is_some_and(|p| p.building == hideout)
        && world.comp::<Brain>(leader).is_some_and(|b| matches!(b.exec, ExecState::Use { kind: ActionKind::Court, .. }))
}

/// Per tick, after the brain (`gang::run`): each waiting order is given at
/// court to a lieutenant there, or by word past the cap, or put in force at
/// once when the gang no longer gives in person (the Statistical mirror);
/// at court the members there hand over the take they hold (P14).
pub fn run(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    for gang in world.gangs() {
        let decided = world.comp::<Gang>(gang).and_then(|g| g.pending_order.as_ref()).map(|p| p.decided);
        if let Some(decided) = decided {
            if !in_person(world, gang) {
                if let Some(p) = world.comp::<Gang>(gang).and_then(|g| g.pending_order.clone()) {
                    crate::systems::faction::apply_order(world, gang, p, None);
                }
            } else if now >= decided + c.order_wait_ticks + c.order_word_ticks {
                let leader = body_leader(world, gang);
                give(world, gang, leader, None, Given::Word);
            }
        }
        let Some(l) = body_leader(world, gang).filter(|&l| holding_court(world, gang, l)) else { continue };
        let Some(hideout) = world.hideout_of(gang) else { continue };
        // P14: the take a member holds, handed over at court in person.
        let holding: Vec<EntityId> = world
            .comp::<Gang>(gang)
            .map(|g| g.members.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&m| m != l && presence::body_at(world, m, hideout))
            .filter(|&m| world.comp::<Brain>(m).is_some_and(|b| b.loot_today > 0))
            .collect();
        for m in holding {
            let share = crate::systems::gang::split_loot(world, m);
            handed(world, m, share);
        }
        if world.comp::<Gang>(gang).is_none_or(|g| g.pending_order.is_none()) {
            continue;
        }
        // The court calls the gang in (the Hideout their top spot to its end).
        let end = now - tod(world) + court_window(&c).1;
        if let Some(g) = world.comp_mut::<Gang>(gang) {
            if g.call_until.is_none_or(|t| t < end) {
                g.call_until = Some(end);
            }
        }
        let day = world.day();
        let seed = world.seed();
        let lt = lieutenants(world, gang).into_iter().find(|&m| {
            if presence::is_body(world, m) {
                presence::body_at(world, m, hideout)
            } else {
                crate::systems::econ::hash_unit(seed, PURPOSE_COURT, u64::from(m.index), day) < c.court_stat_p
            }
        });
        if let Some(lt) = lt {
            give(world, gang, Some(l), Some(lt), Given::Court);
        }
    }
}

/// P14: a member's `SplitLoot` share, handed to the leader holding court
/// at the Hideout the member stands in (the coins went to the treasury as before).
pub fn handed(world: &mut World, id: EntityId, share: i64) {
    if !on(world) || share <= 0 {
        return;
    }
    let Some(gang) = world.gang_of(id) else { return };
    let Some(l) = body_leader(world, gang).filter(|&l| l != id && holding_court(world, gang, l)) else { return };
    if world.comp::<Position>(id).and_then(|p| p.building) != world.hideout_of(gang) {
        return;
    }
    ownership::jobs2_row(world).court_handovers += 1;
    let text = format!("{} handed {share} coins to {} at court", world.name_of(id), world.name_of(l));
    world.push_event(EventKind::TakeHanded, &[id, l, gang], text);
}
