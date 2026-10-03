//! End-of-day snapshot. Runs last in the tick order; at the final tick of
//! each day it fills the snapshot columns of `stats.current` and rolls it
//! into `stats.history`.

use crate::components::{Building, BuildingKind, GangMember, Household, Job, Market, Mood, Needs, Sentence, Treasury};
use crate::time::{self, TICKS_PER_DAY};
use crate::world::World;

pub fn run(world: &mut World) {
    if u64::from(time::tick_of_day(world.tick)) != TICKS_PER_DAY - 1 {
        return;
    }
    snapshot(world);
    let next_day = time::day(world.tick) + 1;
    world.stats.roll(next_day);
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

    let price = world.market().map_or(0, |m: &Market| m.price_food);
    let treasury = world.treasury().map_or(0, |t: &Treasury| t.coins);

    let population = citizens.len() as u32;
    let denom = population.max(1) as f32;
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
    // `goal_changes` is bumped by the think system from M2 on.
    row.goal_changes_per_agent = row.goal_changes as f32 / denom;
}
