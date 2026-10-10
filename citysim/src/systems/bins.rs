//! Hunger as sickness § 4 (docs/HUNGER_SICKNESS.md): the bins behind
//! Markets, eateries and Homes, and the dive.
//!
//! A bin holds meals of scraps (`Building.bin`). Each midnight the day's
//! spoiled units fill the Market and Home bins (`economy::daily_spoilage`),
//! a venue's served meals leave `venue_scrap` each, and every bin rots by
//! `bin_rot`. The dive is a scripted Eat plan (the Mission's pattern:
//! `[GoTo(Bin), DumpsterDive]`, never searched) for a hungry adult who
//! cannot buy a meal; the Statistical tier dives on its row's `p_dive`.
//! Both tiers go through [`contest`] (one diver at a time, the bin's
//! regular, the gang's turf) and [`eat_scraps`].

use crate::components::{
    ActionInstance, BinRegular, Building, BuildingKind, Controller, Health, Lod, MealKind, MemoryKind, Needs,
    Personality, Position, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::Plan;
use crate::goap::{ActionKind, LocationKey};
use crate::time::Tick;
use crate::world::World;

/// `econ::hash_unit` purposes.
const ROLL_SICK: u64 = 0xb1_0001;
const ROLL_SHAKE: u64 = 0xb1_0002;

/// The bins run (`[sickness] enabled`, `diet` and `bins`).
pub fn on(world: &World) -> bool {
    crate::systems::diet::on(world) && world.config.sickness.bins
}

/// The kinds that keep a bin.
pub fn has_bin(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::Market
            | BuildingKind::Home
            | BuildingKind::NoodleBar
            | BuildingKind::Bar
            | BuildingKind::Club
            | BuildingKind::Lounge
            | BuildingKind::FightPit
            | BuildingKind::Den
            | BuildingKind::Arcade
    )
}

/// Add `meals` of scraps to `b`'s bin (capped), keeping `World::bins`.
pub fn add(world: &mut World, b: EntityId, meals: u32) {
    if meals == 0 {
        return;
    }
    let cap = world.config.sickness.bin_cap;
    let Some(bd) = world.comp_mut::<Building>(b) else { return };
    if !has_bin(bd.kind) || bd.demolished {
        return;
    }
    let before = bd.bin;
    bd.bin = (u32::from(bd.bin) + meals).min(u32::from(cap)) as u16;
    let added = u32::from(bd.bin - before);
    if bd.bin > 0 {
        world.bins.insert(b);
    }
    world.stats.current.health.bin_meals_in += added;
}

/// Midnight: a building's units spoiled today, into its bin.
pub fn spoiled(world: &mut World, b: EntityId, units: u32) {
    if !on(world) || units == 0 {
        return;
    }
    let per = u32::from(world.config.sickness.bin_meals_per_unit);
    add(world, b, units * per);
}

/// A meal served at venue `b` (EatOut): `venue_scrap` of a meal into its bin.
pub fn venue_meal(world: &mut World, b: EntityId) {
    let scrap = world.config.sickness.venue_scrap;
    waste(world, b, scrap);
}

/// `units` sold off Market `b`'s shelf: `market_waste` a unit into its bin.
pub fn market_sale(world: &mut World, b: EntityId, units: u32) {
    let scrap = world.config.sickness.market_waste * units as f32;
    waste(world, b, scrap);
}

/// A meal eaten out of Home `b`'s pantry: `home_scrap` into its bin.
pub fn home_meal(world: &mut World, b: EntityId) {
    let scrap = world.config.sickness.home_scrap;
    waste(world, b, scrap);
}

/// `scrap` of a meal into `b`'s bin through its fraction (`bin_accum`).
fn waste(world: &mut World, b: EntityId, scrap: f32) {
    if !on(world) || scrap <= 0.0 {
        return;
    }
    let whole = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.bin_accum += scrap;
        let whole = bd.bin_accum.floor();
        bd.bin_accum -= whole;
        whole as u32
    };
    add(world, b, whole);
}

/// Midnight, after the fill: every bin rots by `bin_rot`.
pub fn rot(world: &mut World) {
    if !on(world) {
        return;
    }
    let rate = world.config.sickness.bin_rot;
    let bins: Vec<EntityId> = world.bins.iter().copied().collect();
    for b in bins {
        let empty = {
            let Some(bd) = world.comp_mut::<Building>(b) else {
                world.bins.remove(&b);
                continue;
            };
            bd.bin -= (f32::from(bd.bin) * rate).floor() as u16;
            bd.bin == 0
        };
        if empty {
            world.bins.remove(&b);
        }
    }
}

/// The street tile a diver stands on: outside `b`'s door.
pub fn bin_tile(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| world.outside_door(bd))
}

/// Is `b`'s bin held by another diver now?
fn held_by_other(world: &World, b: EntityId, id: EntityId) -> bool {
    world.comp::<Building>(b).and_then(|bd| bd.bin_diver).is_some_and(|(d, until)| d != id && until > world.tick)
}

/// The hunger under which `id` dives: `hunger_below` less `0.3 x pride`
/// (the proud dive hungrier; the plan cost of the spec, as a gate).
fn dive_hunger(world: &World, id: EntityId) -> f32 {
    let pride = world.comp::<Personality>(id).map_or(0.5, |p| p.pride);
    world.config.charity.hunger_below - 0.3 * pride
}

/// The bin `id` would dive now: a hungry adult (under [`dive_hunger`])
/// who cannot buy a meal, the nearest stocked bin within
/// `dive_reach_tiles` of where it stands, not held by another diver.
pub fn dive_target(world: &World, id: EntityId) -> Option<EntityId> {
    if !on(world) || !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let hunger = world.comp::<Needs>(id)?.hunger;
    if hunger >= dive_hunger(world, id) {
        return None;
    }
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    if coins >= crate::systems::life::meal_price(world, id) {
        return None;
    }
    // A body dives from where it stands; off screen, from its Home's door
    // (else its tile), as the Mission's reach is read (P5's trace: the
    // off-screen stand-in stands at the phase's door, rarely near a bin).
    let body = world.comp::<crate::components::Brain>(id).is_some_and(|b| b.lod != Lod::Statistical);
    let home_door = world
        .comp::<crate::components::Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door);
    // P5 (the core_year trace: the off-screen starving were mostly squatters
    // with no Home door, and housed agents of the poor blocks whose nearest
    // bin stood past 40 tiles): a squatter reaches from its squat's door,
    // and the starving walk `[life] hungry_reach_mult` further (G2's rule
    // for a Mission).
    let squat_door =
        world.comp::<crate::components::Squatter>(id).and_then(|s| world.comp::<Building>(s.building)).map(|b| b.door);
    let from = match home_door.or(squat_door) {
        Some(d) if !body => d,
        _ => world.comp::<Position>(id)?.tile,
    };
    let mut reach = world.config.sickness.dive_reach_tiles;
    if hunger < world.config.life.starving_hunger {
        reach = (reach as f32 * world.config.life.hungry_reach_mult) as u32;
    }
    world
        .bins
        .iter()
        .copied()
        .filter(|&b| !held_by_other(world, b, id) && !world.is_closed(b))
        .filter_map(|b| bin_tile(world, b).map(|t| (t.manhattan(from), b)))
        .filter(|&(d, _)| d <= reach)
        .min()
        .map(|(_, b)| b)
}

/// The scripted Eat plan: `[GoTo(Bin) b, DumpsterDive b]`.
pub fn dive_plan(world: &World, id: EntityId) -> Option<Plan> {
    let b = dive_target(world, id)?;
    let step = |action, target| ActionInstance { action, target, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Bin), Some(b)), step(ActionKind::DumpsterDive, Some(b))];
    Some(Plan { goal: crate::components::GoalKind::Eat, target: Some(b), steps, started_tick: world.tick })
}

/// `DumpsterDive`'s start check: at the bin's tile, scraps in it, not held.
pub fn can_start(world: &World, id: EntityId, target: Option<EntityId>) -> bool {
    let Some(b) = target else { return false };
    let at = world.comp::<Position>(id).is_some_and(|p| p.building.is_none() && Some(p.tile) == bin_tile(world, b));
    at && world.comp::<Building>(b).is_some_and(|bd| bd.bin > 0) && !held_by_other(world, b, id)
}

/// At start: the diver holds the bin for the dive (one at a time).
pub fn on_start(world: &mut World, id: EntityId, target: Option<EntityId>, until: Tick) {
    if let Some(bd) = target.and_then(|b| world.comp_mut::<Building>(b)) {
        bd.bin_diver = Some((id, until));
    }
}

/// On abort: the bin is let go.
pub fn release(world: &mut World, id: EntityId, target: Option<EntityId>) {
    if let Some(bd) = target.and_then(|b| world.comp_mut::<Building>(b)) {
        if bd.bin_diver.is_some_and(|(d, _)| d == id) {
            bd.bin_diver = None;
        }
    }
}

/// `DumpsterDive` at completion: the contest, then the meal.
pub fn on_complete(world: &mut World, id: EntityId, target: Option<EntityId>) -> crate::exec::StepResult {
    use crate::exec::{FailReason, StepResult};
    let Some(b) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
    release(world, id, Some(b));
    if dive(world, id, b) {
        StepResult::Done
    } else {
        StepResult::Failed(FailReason::StockGone)
    }
}

/// How a contest at the bin ended.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Contest {
    /// The diver eats.
    Clear,
    /// The bin's regular saw the diver off.
    LostToRegular(EntityId),
    /// A gang's member (or its turf, off screen) shook the diver down.
    Shaken,
}

/// The gang whose street `b`'s bin stands on: the gang holding the nearest
/// held Home (`gang::CLAIM_HELD`) within `bin_turf_tiles` of the bin, else
/// the gang controlling the district outright. A district is almost never a
/// gang's (P3 measured: Corp, City or Contested on the flipped seed 42), so
/// the held Home is the turf a diver meets.
pub fn turf(world: &World, b: EntityId) -> Option<EntityId> {
    let at = bin_tile(world, b)?;
    let reach = world.config.sickness.bin_turf_tiles;
    let d = world.district_of_building(b);
    let dist = world.districts.get(d.index())?;
    let held = dist
        .homes
        .iter()
        .filter_map(|&h| world.comp::<Building>(h).map(|bd| (h, bd)))
        .filter_map(|(h, bd)| {
            let c = bd.claim?;
            (c.count >= crate::systems::gang::CLAIM_HELD && world.has::<crate::components::Gang>(c.gang))
                .then(|| (bd.door.manhattan(at), h.index, c.gang))
        })
        .filter(|&(dd, _, _)| dd <= reach)
        .min()
        .map(|(_, _, g)| g);
    held.or(match dist.control {
        Controller::Gang(g) if world.has::<crate::components::Gang>(g) => Some(g),
        _ => None,
    })
}

/// A member of `gang` within sight of `tile` (a body), if any.
fn member_near(world: &World, gang: EntityId, tile: TilePos) -> Option<EntityId> {
    let sight = world.config.crime.sight;
    let g = world.comp::<crate::components::Gang>(gang)?;
    g.members.iter().copied().find(|&m| {
        crate::systems::law::living(world, m)
            && world.comp::<Position>(m).is_some_and(|p| p.building.is_none() && p.tile.manhattan(tile) <= sight)
    })
}

/// § 4: one function for both tiers. The bin's regular (a diver there on
/// `regular_days` of the last 7) present (a body within 3 tiles, or off
/// screen a diver there today) puts an Intimidate to the newcomer
/// (`moves::resolve`; a failed one may backlash into a fight: the normal
/// combat rolls, decision 8); the loser goes without, the pair's affinity
/// drifts and a second loss makes them Rivals. On a gang's turf a
/// non-member is shaken down (`bin_shake_p`, a pure hash, once a day; the
/// turf is [`turf`]): a member on the street within sight (Full) Intimidates for
/// `bin_toll` and, against a diver with nothing to pay, beats it (an
/// Assault through `law::resolve_fight`); off screen the toll is paid or
/// the meal lost. Gang members dive free.
pub fn contest(world: &mut World, id: EntityId, b: EntityId) -> Contest {
    let day = world.day();
    let full = world.comp::<crate::components::Brain>(id).is_some_and(|br| br.lod != Lod::Statistical);
    let tile = bin_tile(world, b).unwrap_or_default();
    // The regular.
    let regular = world.comp::<Building>(b).and_then(|bd| bd.bin_regular);
    if let Some(r) = regular.filter(|r| r.who != id && r.established(day, world.config.sickness.regular_days)) {
        let present = crate::systems::law::living(world, r.who)
            && match world.comp::<crate::components::Brain>(r.who).map(|br| br.lod) {
                Some(Lod::Statistical) | None => r.last_day == day,
                _ => world.comp::<Position>(r.who).is_some_and(|p| p.tile.manhattan(tile) <= 3),
            };
        if present {
            let m = crate::word::SocialMove {
                actor: r.who,
                target: id,
                kind: crate::word::MoveKind::Intimidate,
                stake: crate::word::Stake::Coins(0),
            };
            let out = crate::systems::moves::resolve(world, &m);
            let (winner, loser) = if out.success { (r.who, id) } else { (id, r.who) };
            lost_bin(world, winner, loser, b);
            if out.success {
                return Contest::LostToRegular(r.who);
            }
        }
    }
    // The gang's turf.
    if let Some(gang) = turf(world, b) {
        if world.gang_of(id) != Some(gang) && world.comp::<Health>(id).is_none_or(|h| h.shaken_day != Some(day)) {
            let p = world.config.sickness.bin_shake_p;
            let roll = crate::systems::econ::hash_unit(world.seed(), ROLL_SHAKE, u64::from(id.index), day);
            if roll < p {
                let member = if full { member_near(world, gang, tile) } else { None };
                if full && member.is_none() {
                    return Contest::Clear;
                }
                if let Some(h) = world.comp_mut::<Health>(id) {
                    h.shaken_day = Some(day);
                }
                world.stats.current.health.bin_shakedowns += 1;
                let toll = world.config.sickness.bin_toll;
                let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
                let name = world.name_of(id);
                if coins >= toll {
                    crate::systems::ownership::pay(
                        world,
                        Some(id),
                        Some(gang),
                        toll,
                        crate::systems::ownership::Flow::Tribute,
                    );
                    world.push_event(
                        EventKind::BinContest,
                        &[gang, id],
                        format!("{name} paid the gang to eat from the bin"),
                    );
                    return Contest::Clear;
                }
                if let Some(mem) = member {
                    let (winner, loser, died) = crate::systems::law::resolve_fight(world, mem, id);
                    let crime = if died && loser == id {
                        crate::components::Crime::Murder
                    } else {
                        crate::components::Crime::Assault
                    };
                    let text = format!("{} ran {name} off the gang's bin", world.name_of(mem));
                    world.push_event(
                        if crime == crate::components::Crime::Murder { EventKind::Murder } else { EventKind::Assault },
                        &[mem, id],
                        text,
                    );
                    if crate::systems::law::living(world, mem) {
                        crate::systems::law::raise_crime_on(world, mem, Some(id), None, crime, tile);
                    }
                    if winner == id {
                        return Contest::Clear;
                    }
                } else {
                    world.push_event(EventKind::BinContest, &[gang, id], format!("{name} was run off the gang's bin"));
                }
                return Contest::Shaken;
            }
        }
    }
    Contest::Clear
}

/// The loser of a bin contest goes without: an event, a memory, the drift.
fn lost_bin(world: &mut World, winner: EntityId, loser: EntityId, b: EntityId) {
    world.stats.current.health.bin_contests += 1;
    let text = format!("{} saw {} off the bin behind #{}", world.name_of(winner), world.name_of(loser), b.index);
    world.push_event(EventKind::BinContest, &[winner, loser, b], text);
    world.remember(loser, MemoryKind::Rejected, Some(winner), 0.4, -0.3, false);
    crate::systems::social::adjust(world, loser, winner, -0.1, -0.1);
    // A second loss to the same rival: Rivals.
    let rival = world.edge(loser, winner).is_some_and(|e| e.affinity <= -0.15);
    if rival {
        let e = world.edge_entry(loser, winner);
        if matches!(e.kind, crate::components::RelKind::Acquaintance | crate::components::RelKind::Friend) {
            e.kind = crate::components::RelKind::Rival;
        }
    }
}

/// The dive: the contest, then one meal of scraps eaten at the bin. False
/// when the bin is empty or the contest was lost.
pub fn dive(world: &mut World, id: EntityId, b: EntityId) -> bool {
    if world.comp::<Building>(b).is_none_or(|bd| bd.bin == 0) {
        return false;
    }
    if contest(world, id, b) != Contest::Clear || !crate::systems::law::living(world, id) {
        return false;
    }
    eat_scraps(world, id, b);
    true
}

/// One meal of scraps off `b`'s bin into `id` (both tiers).
fn eat_scraps(world: &mut World, id: EntityId, b: EntityId) {
    let day = world.day();
    let need = world.config.sickness.regular_days;
    let empty = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.bin -= 1;
        match bd.bin_regular.as_mut() {
            Some(r) if r.who == id => r.saw(day),
            Some(r) if !r.established(day, need) => *r = BinRegular::new(id, day),
            None => bd.bin_regular = Some(BinRegular::new(id, day)),
            _ => {}
        }
        bd.bin == 0
    };
    if empty {
        world.bins.remove(&b);
    }
    let cfg = world.config.needs.clone();
    let satisfy = world.config.sickness.dive_satisfy;
    if let Some(n) = world.comp_mut::<Needs>(id) {
        n.hunger = (n.hunger + cfg.food_satisfy * satisfy).min(1.0);
    }
    crate::systems::health::ate(world, id, MealKind::Dive);
    world.mark_day(id, crate::components::trace_flags::ATE);
    let pride = world.comp::<Personality>(id).map_or(0.5, |p| p.pride);
    world.remember(id, MemoryKind::Ate, None, 0.2, -0.2 * pride, false);
    // Food poisoning (a pure hash: no stream moves).
    let roll = crate::systems::econ::hash_unit(world.seed(), ROLL_SICK, u64::from(id.index), world.tick);
    if roll < world.config.sickness.dive_sick_p {
        if let Some(n) = world.comp_mut::<Needs>(id) {
            n.energy = (n.energy - 0.2).max(0.0);
        }
    }
    if let Some(t) = bin_tile(world, b) {
        crate::systems::litter::deposit(world, t, 2, 0);
    }
}

/// Off screen (`lod::stat_eat`, an adult who cannot buy a meal): a dive on
/// `p` of hours (the row's learned `p_dive`), the same bin choice, contest
/// and meal as a body's. True when it ate.
pub fn stat_dive(world: &mut World, id: EntityId, p: f32) -> bool {
    let Some(b) = dive_target(world, id) else { return false };
    let u = crate::systems::econ::hash_unit(world.seed(), ROLL_SICK ^ 0xff, u64::from(id.index), world.tick);
    if u >= p {
        return false;
    }
    dive(world, id, b)
}
