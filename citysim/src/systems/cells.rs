//! V8 (docs/VIEW_MILESTONE.md § V8, `OPEN-256`): the cells' week.
//!
//! The Precinct swallowed the week at the walking pace: every short term
//! served to its last night, the beds per capita (`[buildings] jail capacity`,
//! 8 % of the city) never full, a cell day the same as the last. Dylan: a
//! feature, not a knob.
//!
//! - **Fines at booking** ([`fine_out`], from `law::jail_suspect_at`): a term
//!   of `fine_max_days` or fewer is bought out for `fine_per_day` coins a day
//!   of it from the purse: the convict's own, then his kin's (a spouse, a
//!   parent, a grown child; never a child's), above `keep_coins` (the week's
//!   food), to the city (`Flow::Fine`). The booking stands (the record, the
//!   interrogation); the cell does not.
//! - **Bail** ([`daily`], the LivingEarly midnight, after the gangs' bail in
//!   `protection::daily`): a prisoner with `bail_max_days` or fewer left on a
//!   term of `bail_term_max_days` or fewer is bailed for `bail_per_day` coins a
//!   day left from the same purses.
//! - **Overcrowding** ([`daily`]): past `crowd_from` of the Precinct's beds
//!   held, the least dangerous (the pettiest crime, then the soonest out) go
//!   early until `crowd_to`; a line each.
//! - **The cell day** ([`hourly`], [`cards`]): the work detail at `work_hour`
//!   (`work_p` of the prisoners a day, one of `work_details`, `work_pay` from
//!   the city, `Flow::Wage`); cards in the yard with a cellmate (`cards_p`, the
//!   stake `Flow::Gamble`), besides X6's meal, walk and fights and the kin's
//!   visits.
//! - **BreakOut** ([`breakout_reach`], from `faction::apply_order`): an order
//!   its gang cannot march is dropped as it is chosen, a line why.
//!
//! Every coin moves between purses (`ownership::pay`): none made or lost.
//! Under 18: a child is never held (`ages::is_protected`), never pays, never
//! plays. Statistical mirror: tier-blind (the held prisoners are in
//! `World::sentenced` as the bodies are), the kin's purses read alike.

use crate::components::{BuildingKind, Identity, Position, RelKind, Sentence};
use crate::config::CellsCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

const P_WORK: u64 = 0x0c0e_0001;
const P_DETAIL: u64 = 0x0c0e_0002;
const P_CARDS: u64 = 0x0c0e_0003;
const P_HAND: u64 = 0x0c0e_0004;

pub fn cfg(world: &World) -> Option<&CellsCfg> {
    world.config.cells.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

/// "1 day", "3 days".
fn n_days(n: u64) -> String {
    if n == 1 {
        "1 day".to_string()
    } else {
        format!("{n} days")
    }
}

fn person(world: &World, p: EntityId) -> bool {
    world.has::<Identity>(p) && crate::systems::law::living(world, p)
}

/// "his", "her".
fn his(world: &World, p: EntityId) -> &'static str {
    match world.comp::<Identity>(p).map(|i| i.sex) {
        Some(crate::components::Sex::Female) => "her",
        _ => "his",
    }
}

/// The purses that pay for `p`: his own, then his kin's (a spouse, a parent or
/// a grown child, family), free living adults, the spouse first.
fn payers(world: &World, p: EntityId) -> Vec<(EntityId, &'static str)> {
    let mut kin: Vec<(u8, EntityId, &'static str)> = world
        .neighbours(p)
        .filter(|&o| person(world, o) && !world.has::<Sentence>(o) && !crate::systems::ages::is_protected(world, o))
        .filter_map(|o| {
            let e = world.edge(p, o)?;
            match e.kind {
                RelKind::Spouse => Some((0, o, "spouse")),
                RelKind::Parent => Some((1, o, "kin")),
                RelKind::Family => Some((2, o, "kin")),
                _ => None,
            }
        })
        .collect();
    kin.sort_unstable();
    std::iter::once((p, "own")).chain(kin.into_iter().map(|(_, o, w)| (o, w))).collect()
}

/// Pay `amount` for `p` from the first purse holding it above `keep_coins`
/// (to the city, `Flow::Fine`). The payer and his words ("from his own purse",
/// "paid by his wife Ada Kemp"), or `None`.
fn pay_for(world: &mut World, c: &CellsCfg, p: EntityId, amount: i64) -> Option<(EntityId, String)> {
    if amount <= 0 {
        return None;
    }
    for (x, who) in payers(world, p) {
        if world.purse(Some(x)) - c.keep_coins < amount {
            continue;
        }
        if ownership::pay(world, Some(x), None, amount, Flow::Fine) < amount {
            continue;
        }
        let words = if who == "own" {
            format!("from {} own purse", his(world, p))
        } else {
            let rel = if who == "spouse" { "spouse" } else { "kin" };
            format!("paid by {} {rel} {}", his(world, p), world.name_of(x))
        };
        return Some((x, words));
    }
    None
}

/// At booking (`law::jail_suspect_at`, the cells' room settled): a term of
/// `term` ticks bought out from the purse. True: fined, no cell (the caller
/// books him and lets him go at the door).
pub fn fine_out(world: &mut World, suspect: EntityId, crime: crate::components::Crime, term: Tick) -> bool {
    let Some(c) = cfg(world).cloned() else { return false };
    if c.fine_max_days == 0 || crate::systems::ages::is_protected(world, suspect) {
        return false;
    }
    let days = term.div_ceil(TICKS_PER_DAY);
    if days == 0 || days > u64::from(c.fine_max_days) {
        return false;
    }
    let amount = c.fine_per_day * days as i64;
    let charge = crate::systems::law::charge_label(world, suspect, crime);
    let Some((payer, words)) = pay_for(world, &c, suspect, amount) else { return false };
    let mut actors = vec![suspect];
    if payer != suspect {
        actors.push(payer);
    }
    let text = format!(
        "{} fined {amount}c for {charge} instead of {} in the cells, {words}",
        world.name_of(suspect),
        n_days(days)
    );
    world.push_event(EventKind::Sentence, &actors, text);
    let row = ownership::jobs2_row(world);
    row.cells_fined += 1;
    row.cells_fine_coins += amount.max(0) as u32;
    true
}

/// The LivingEarly midnight: the bail, then the crowded Precinct's early
/// releases.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    if c.bail_max_days > 0 && c.bail_per_day > 0 {
        let held: Vec<EntityId> = world.sentenced().to_vec();
        for p in held {
            if !person(world, p) {
                continue;
            }
            let Some(s) = world.comp::<Sentence>(p).cloned() else { continue };
            let left = s.until_tick.saturating_sub(now);
            let term = crate::systems::law::sentence_ticks(world, s.crime);
            if left == 0
                || left > u64::from(c.bail_max_days) * TICKS_PER_DAY
                || term > u64::from(c.bail_term_max_days) * TICKS_PER_DAY
            {
                continue;
            }
            let days = left.div_ceil(TICKS_PER_DAY);
            let amount = c.bail_per_day * days as i64;
            let Some((payer, words)) = pay_for(world, &c, p, amount) else { continue };
            let from = jail_label(world, p);
            let mut actors = vec![p];
            if payer != p {
                actors.push(payer);
            }
            let text = format!(
                "{} bailed out of the {from} for {amount}c ({} left for {}), {words}",
                world.name_of(p),
                n_days(days),
                s.crime.label()
            );
            world.push_event(EventKind::Release, &actors, text);
            let row = ownership::jobs2_row(world);
            row.cells_bailed += 1;
            row.cells_bail_coins += amount.max(0) as u32;
            crate::systems::law::release(world, p, false);
        }
    }
    crowding(world, &c);
}

fn jail_label(world: &World, p: EntityId) -> &'static str {
    let station = world
        .comp::<Position>(p)
        .and_then(|x| x.building)
        .is_some_and(|b| crate::systems::stations::is_station(world, b));
    if station {
        BuildingKind::Station.label()
    } else {
        "Precinct"
    }
}

/// Overcrowding: past `crowd_from` of the beds held, the pettiest (then the
/// soonest out, the lower id) go early until `crowd_to`.
fn crowding(world: &mut World, c: &CellsCfg) {
    if c.crowd_from <= 0.0 {
        return;
    }
    let beds = usize::from(world.config.buildings.jail.capacity);
    let held = crate::systems::stations::precinct_held(world);
    let from = (beds as f32 * c.crowd_from).ceil() as usize;
    if beds == 0 || held <= from {
        return;
    }
    let to = (beds as f32 * c.crowd_to.min(c.crowd_from)).floor() as usize;
    let Some(precinct) = world.building_of_kind(BuildingKind::Jail) else { return };
    let mut least: Vec<(crate::components::Crime, Tick, EntityId)> = world
        .sentenced()
        .iter()
        .copied()
        .filter(|&p| person(world, p))
        .filter(|&p| world.comp::<Position>(p).and_then(|x| x.building) == Some(precinct))
        .filter_map(|p| world.comp::<Sentence>(p).map(|s| (s.crime, s.until_tick, p)))
        .collect();
    least.sort_unstable();
    let n = held.saturating_sub(to);
    let pname = world.name_of(precinct);
    for (crime, until, p) in least.into_iter().take(n) {
        let left = until.saturating_sub(world.tick).div_ceil(TICKS_PER_DAY);
        let text = format!(
            "{} let out early from {pname}, {} short on {} {}: the cells crowded ({held} held, over {from})",
            world.name_of(p),
            n_days(left),
            his(world, p),
            crime.label()
        );
        world.push_event(EventKind::Release, &[p, precinct], text);
        ownership::jobs2_row(world).cells_early += 1;
        crate::systems::law::release(world, p, false);
    }
}

/// Hourly (from `living::run`): the work detail at `work_hour`.
pub fn hourly(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    if c.work_hour >= 24 || c.work_details.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if world.tick_of_day() / TICKS_PER_HOUR as u16 != u16::from(c.work_hour) {
        return;
    }
    let day = world.day();
    let held: Vec<EntityId> = world.sentenced().to_vec();
    for p in held {
        if !person(world, p) || crate::systems::ages::is_protected(world, p) {
            continue;
        }
        let Some(jail) = world.comp::<Position>(p).and_then(|x| x.building) else { continue };
        if hash(world, P_WORK, p, day) >= c.work_p {
            continue;
        }
        let i = ((hash(world, P_DETAIL, p, day) * c.work_details.len() as f32) as usize).min(c.work_details.len() - 1);
        let detail = c.work_details[i].clone();
        let paid = if c.work_pay > 0 { ownership::pay(world, None, Some(p), c.work_pay, Flow::Wage) } else { 0 };
        let pay = if paid > 0 { format!(" for {paid}c") } else { " unpaid: the city broke".to_string() };
        let text = format!("{} worked {detail} of {}{pay}", world.name_of(p), world.name_of(jail));
        world.push_event(EventKind::Cells, &[p, jail], text);
        ownership::jobs2_row(world).cell_work += 1;
    }
}

/// The yard (`consequence::hourly`, before the walk): `p` with nothing else
/// on plays cards with a cellmate free of the day's yard (`cards_p`); the
/// better hand a draw, `cards_stake` changing hands. True: played (both
/// taken).
pub fn cards(
    world: &mut World,
    yard: &str,
    jail: EntityId,
    p: EntityId,
    mates: &[EntityId],
    taken: &mut std::collections::BTreeSet<EntityId>,
) -> bool {
    let Some(c) = cfg(world).cloned() else { return false };
    let day = world.day();
    if c.cards_p <= 0.0 || hash(world, P_CARDS, p, day) >= c.cards_p || crate::systems::ages::is_protected(world, p) {
        return false;
    }
    let free: Vec<EntityId> = mates
        .iter()
        .copied()
        .filter(|&m| m != p && !taken.contains(&m) && person(world, m) && world.has::<Sentence>(m))
        .filter(|&m| !crate::systems::ages::is_protected(world, m))
        .collect();
    if free.is_empty() {
        return false;
    }
    let roll = hash(world, P_HAND, p, day);
    let m = free[((roll * free.len() as f32) as usize).min(free.len() - 1)];
    let (winner, loser) = if hash(world, P_HAND, m, day ^ 0x5a5a) < 0.5 { (p, m) } else { (m, p) };
    let won = if c.cards_stake > 0 {
        ownership::pay(world, Some(loser), Some(winner), c.cards_stake, Flow::Gamble)
    } else {
        0
    };
    crate::systems::social::interacted(world, p, m);
    let end = if won > 0 {
        format!("{} took {won}c off {}", world.name_of(winner), world.name_of(loser))
    } else {
        format!("{} won the hand, {} had nothing to lose", world.name_of(winner), world.name_of(loser))
    };
    let text = format!("{} played cards with {} in {yard}: {end}", world.name_of(p), world.name_of(m));
    world.push_event(EventKind::Cells, &[p, m, jail], text);
    taken.insert(p);
    taken.insert(m);
    ownership::jobs2_row(world).cell_cards += 1;
    true
}

/// BreakOut (`faction::apply_order`): the order its gang can march, or the
/// line why not. `None`: it can (or the gate is off); `Some(why)`: fewer than
/// `[gangs] raid_min_members` of its free adults within `[crew]
/// duty_reach_ticks` of the Precinct (from their Homes), the jailed named.
pub fn breakout_reach(world: &World, gang: EntityId) -> Option<String> {
    let c = cfg(world)?;
    if !c.breakout_reach {
        return None;
    }
    let reach = crate::systems::crew::cfg(world)?.duty_reach_ticks;
    let jail = world.building_of_kind(BuildingKind::Jail)?;
    let door = world.comp::<crate::components::Building>(jail)?.door;
    let members = world.comp::<crate::components::Gang>(gang)?.members.clone();
    let near = members
        .iter()
        .copied()
        .filter(|&m| crate::systems::contracts::free_adult(world, m))
        .filter(|&m| {
            crate::systems::contracts::home_door(world, m)
                .is_some_and(|from| crate::systems::distance::trip_ahead(world, m, from, door) <= reach)
        })
        .count();
    let min = world.config.gangs.raid_min_members.max(1);
    if near >= min {
        return None;
    }
    let inside: Vec<String> =
        members.iter().copied().filter(|&m| world.has::<Sentence>(m)).take(3).map(|m| world.name_of(m)).collect();
    let who = if inside.is_empty() { "its own".to_string() } else { inside.join(", ") };
    Some(format!(
        "would break {who} out of {}, but {near} of its free members within reach of it (it takes {min}): the break-out dropped",
        world.name_of(jail)
    ))
}
