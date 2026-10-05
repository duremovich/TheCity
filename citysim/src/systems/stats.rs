//! End-of-day snapshot. Runs last in the tick order; at the final tick of
//! each day it writes every adult's `Trace` entry (M10), fills the snapshot
//! columns of `stats.current` and rolls it into `stats.history`.

use crate::components::{
    trace_flags, Brain, Building, BuildingKind, DayTrace, GangMember, Household, Job, Lod, Mood, Needs, Position,
    Sentence, Trace, Treasury,
};
use crate::entity::EntityId;
use crate::time::{self, TICKS_PER_DAY};
use crate::world::World;

pub fn run(world: &mut World) {
    if u64::from(time::tick_of_day(world.tick)) != TICKS_PER_DAY - 1 {
        return;
    }
    record_traces(world);
    snapshot(world);
    let next_day = time::day(world.tick) + 1;
    world.stats.roll(next_day);
}

/// Today's trace entry for a living agent, from live state and today's marks.
pub fn live_day_trace(world: &World, id: EntityId) -> DayTrace {
    let today = world.day();
    let mut flags = trace_flags::ALIVE;
    if world.has::<Sentence>(id) {
        flags |= trace_flags::JAILED;
    }
    if world.comp::<Household>(id).is_none_or(|h| h.home.is_none()) {
        flags |= trace_flags::HOMELESS;
    }
    if world.has::<Job>(id) {
        flags |= trace_flags::EMPLOYED;
    }
    if world.has::<GangMember>(id) {
        flags |= trace_flags::GANG;
    }
    let body_today = world.comp::<Brain>(id).is_some_and(|b| b.body_day == Some(today) || b.lod != Lod::Statistical);
    if !body_today {
        flags |= trace_flags::STATISTICAL_ALL_DAY;
    }
    flags |= world.day_marks.get(&id).copied().unwrap_or(0);
    let zone = world.comp::<Position>(id).map_or_else(Default::default, |p| world.map.zone(p.tile));
    DayTrace {
        zone,
        flags,
        hunger: DayTrace::hunger_band(world.comp::<Needs>(id).map_or(1.0, |n| n.hunger)),
        mood: DayTrace::mood_band(world.comp::<Mood>(id).map_or(0.0, |m| m.value)),
    }
}

/// M10: append today's `DayTrace` to every living adult's `Trace`, then clear
/// the day marks and roll the zone watch.
pub fn record_traces(world: &mut World) {
    let today = world.day();
    let cap = world.config.lod.trace_days;
    // scan-ok: daily: traces
    for id in world.citizens() {
        if !world.has::<Brain>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let t = live_day_trace(world, id);
        if !world.has::<Trace>(id) {
            world.insert(id, Trace::default());
        }
        if let Some(tr) = world.comp_mut::<Trace>(id) {
            tr.push(today, t, cap);
        }
    }
    world.day_marks.clear();
    world.zone_watch.yesterday = std::mem::take(&mut world.zone_watch.today);
}

/// Fill the snapshot columns of the current day from live world state.
pub fn snapshot(world: &mut World) {
    let citizens = world.citizens();
    let mut employed = 0;
    let mut homeless = 0;
    let mut jailed = 0;
    let mut gang_members = 0;
    let mut hunger_sum = 0.0f32;
    let mut mood_sum = 0.0f32;
    for &id in &citizens {
        if world.has::<Job>(id) {
            employed += 1;
        }
        if matches!(world.comp::<Household>(id), Some(Household { home: None })) {
            homeless += 1;
        }
        if world.has::<Sentence>(id) {
            jailed += 1;
        }
        if world.has::<GangMember>(id) {
            gang_members += 1;
        }
        if let Some(n) = world.comp::<Needs>(id) {
            hunger_sum += n.hunger;
        }
        if let Some(m) = world.comp::<Mood>(id) {
            mood_sum += m.value;
        }
    }

    let mut food_market = 0;
    let mut food_warehouse = 0;
    let mut food_pantry = 0;
    for id in world.with::<Building>() {
        let Some(b) = world.comp::<Building>(id) else { continue };
        match b.kind {
            BuildingKind::Market => food_market += b.stock_food,
            BuildingKind::Warehouse => food_warehouse += b.stock_food,
            BuildingKind::Home => food_pantry += b.stock_food,
            _ => {}
        }
    }

    let price = world.mean_price();
    let treasury = world.treasury().map_or(0, |t: &Treasury| t.coins);

    let population = citizens.len() as u32;
    let denom = population.max(1) as f32;
    // Goal changes are per agent that thinks (Statistical agents have no goals).
    let thinking = citizens
        .iter()
        .filter(|&&id| {
            world.comp::<crate::components::Brain>(id).is_some_and(|b| b.lod != crate::components::Lod::Statistical)
        })
        .count()
        .max(1) as f32;
    let (holes_open, tiers) =
        (world.holes.len() as u32, [Lod::Full, Lod::Coarse, Lod::Statistical].map(|l| world.tier(l).len() as u32));
    let row = &mut world.stats.current;
    row.population = population;
    row.employed = employed;
    row.homeless = homeless;
    row.jailed = jailed;
    row.gang_members = gang_members;
    row.food_market = food_market;
    row.food_warehouse = food_warehouse;
    row.food_pantry = food_pantry;
    row.price = price;
    row.treasury = treasury;
    row.mean_hunger = hunger_sum / denom;
    row.mean_mood = mood_sum / denom;
    row.goal_changes_per_agent = row.goal_changes as f32 / thinking;
    row.holes_open = holes_open;
    row.tier_full = tiers[0];
    row.tier_coarse = tiers[1];
    row.tier_stat = tiers[2];
}
