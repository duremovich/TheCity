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
    let tile = world.comp::<Position>(id).map(|p| p.tile);
    let zone = tile.map_or_else(Default::default, |t| world.map.zone(t));
    let district = tile.map_or(crate::components::DistrictId::UNSET, |t| world.district_of(t));
    DayTrace {
        zone,
        district,
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
    world.district_watch.yesterday = std::mem::take(&mut world.district_watch.today);
    // M11 D34: the guard-hours near each Home roll with the zone watch.
    world.home_watch.yesterday = std::mem::take(&mut world.home_watch.today);
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
        if matches!(world.comp::<Household>(id), Some(Household { home: None, .. })) {
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
    let mut slots = vec![None; crate::stats::CORP_SLOTS];
    for c in world.corps() {
        if let Some(cc) = world.comp::<crate::components::Corp>(c) {
            if let Some(s) = cc.slot.map(usize::from).filter(|&s| s < slots.len()) {
                slots[s] = Some((cc.treasury, cc.order));
            }
        }
    }
    world.stats.current.corps = slots;
    let cls = world.classes.clone();
    let row = &mut world.stats.current;
    row.unrest_corp = cls[0].unrest;
    row.unrest_street = cls[1].unrest;
    row.unrest_dreg = cls[2].unrest;
    row.class_corp = cls[0].count;
    row.class_street = cls[1].count;
    row.class_dreg = cls[2].count;
    row.happiness_street = cls[1].happiness;
    // M12 D45: the district slots and the city-wide street/riot columns.
    let slots: Vec<crate::stats::DistrictCols> = world
        .districts
        .iter()
        .take(crate::stats::DISTRICT_SLOTS)
        .map(|d| (d.coverage, d.control.csv_code(), d.litter, d.unrest, d.crime_rate, d.guards))
        .collect();
    let gangs = world.gang_list().len() as u32;
    let (squatters, derelicts) = crate::systems::street::snapshot(world);
    let row = &mut world.stats.current;
    row.districts = slots;
    row.gangs = gangs;
    row.squatters = squatters;
    row.derelicts = derelicts;
    let mut coins: Vec<i64> = citizens
        .iter()
        .filter(|&&id| world.has::<Brain>(id) && crate::systems::demography::is_adult(world, id))
        .filter_map(|&id| world.comp::<crate::components::Wallet>(id).map(|w| w.coins.max(0)))
        .collect();
    let (gini, top10) = wealth_spread(&mut coins);
    let row = &mut world.stats.current;
    row.wallets = coins.iter().sum();
    row.wallet_gini = gini;
    row.wallet_top10 = top10;
    // M13 D49 snapshots (0 with assets off, D50).
    if world.config.assets.enabled {
        let (chrome, sanity, robots, vehicles) = crate::systems::assets::snapshot(world);
        let legal = u32::from(world.levers.stims_legal);
        let hooked = crate::systems::stims::hooked_count(world);
        let row = &mut world.stats.current;
        row.chrome_agents = chrome;
        row.mean_sanity = sanity;
        row.robots = robots;
        [row.vehicles_moto, row.vehicles_car, row.vehicles_truck, row.vehicles_flyer] = vehicles;
        row.stims_legal = legal;
        row.hooked = hooked;
        // D49: the day's commutes, ticks per Manhattan tile, walked and driven.
        let acc = std::mem::take(&mut world.commute_acc);
        let tpt = |ticks: u64, tiles: u64| if tiles > 0 { ticks as f32 / tiles as f32 } else { 0.0 };
        let row = &mut world.stats.current;
        row.commute_tpt_walk = tpt(acc[0], acc[1]);
        row.commute_tpt_drive = tpt(acc[2], acc[3]);
    }
    // M14 V43 snapshots (0 with the plane off, V44).
    if world.config.virt.enabled {
        virt_snapshot(world);
    }
}

/// M14 V43: alive nodes, standing Labs, the mean effective ICE over alive
/// corp building nodes, the Data held on every alive node, and per corp
/// slot its tiers and Data holding.
fn virt_snapshot(world: &mut World) {
    use crate::virt::{NodeId, NodeKind, OwnerTag, Track};
    let nodes = world.virt.alive_count() as u32;
    let labs = world
        .buildings_of_kind(BuildingKind::Lab)
        .iter()
        .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .count() as u32;
    let (mut ice, mut n_ice, mut held) = (0u32, 0u32, 0u32);
    for (i, n) in world.virt.nodes.iter().enumerate().filter(|(_, n)| n.alive) {
        held += n.store.total();
        if n.owner_kind == OwnerTag::Corp && matches!(n.kind, NodeKind::Building(_)) {
            ice += u32::from(crate::systems::virt::ice_eff(world, NodeId(i as u16)));
            n_ice += 1;
        }
    }
    let mut corps = vec![[0u32; 4]; crate::stats::CORP_SLOTS];
    for c in world.corps() {
        let Some(cc) = world.comp::<crate::components::Corp>(c) else { continue };
        let Some(s) = cc.slot.map(usize::from).filter(|&s| s < corps.len()) else { continue };
        let data: u32 = Track::ALL.iter().map(|&t| crate::systems::virt::holding(world, c, t)).sum();
        let t = cc.tech.tier;
        corps[s] = [u32::from(t[0]), u32::from(t[1]), u32::from(t[2]), data];
    }
    let v = &mut world.stats.current.virt;
    v.nodes = nodes;
    v.labs = labs;
    v.ice_mean_corp = if n_ice > 0 { ice as f32 / n_ice as f32 } else { 0.0 };
    v.data_held = held;
    v.corps = corps;
}

/// `(Gini, the richest tenth's share)` of non-negative holdings; sorts `v`.
pub fn wealth_spread(v: &mut [i64]) -> (f32, f32) {
    v.sort_unstable();
    let n = v.len();
    let total: i64 = v.iter().sum();
    if n == 0 || total <= 0 {
        return (0.0, 0.0);
    }
    // Gini = sum_i (2i - n - 1) x_i / (n sum x), i from 1, ascending.
    let weighted: i128 =
        v.iter().enumerate().map(|(i, &x)| (2 * (i as i128 + 1) - n as i128 - 1) * i128::from(x)).sum();
    let gini = weighted as f64 / (n as f64 * total as f64);
    let top = n.div_ceil(10);
    let top_sum: i64 = v[n - top..].iter().sum();
    (gini as f32, (top_sum as f64 / total as f64) as f32)
}
