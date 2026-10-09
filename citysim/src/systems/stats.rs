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
    // Real economy (plan E44): the World's books close with the day.
    crate::systems::world_market::close_day(world);
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
    // M16a (plan C38): the contract board's snapshot columns.
    crate::systems::contracts::snapshot(world);
    // Jobs v2 (P5a): the civic headcounts the budget (or a god) set.
    world.stats.current.jobs.guard_count = world.levers.guard_count;
    world.stats.current.jobs.sanitation_count = world.levers.sanitation_count;
    // Jobs v2 (P3): the trades' Job holders.
    world.stats.current.jobs.trades_employed =
        world.config.trade_roles().into_iter().map(|r| world.workers(r).len() as u32).sum();
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
    let (holes_open, mut tiers) =
        (world.holes.len() as u32, [Lod::Full, Lod::Coarse, Lod::Statistical].map(|l| world.tier(l).len() as u32));
    // L2 phase 3 (L21, L34): the sentenced by tier; with the budget on they
    // count in `tier_held`, not in the three tiers.
    let mut held_by_tier = [0u32; 3];
    for &p in world.sentenced() {
        if let Some(b) = world.comp::<Brain>(p) {
            held_by_tier[b.lod as usize] += 1;
        }
    }
    for (t, h) in tiers.iter_mut().zip(held_by_tier) {
        *t = t.saturating_sub(h);
    }
    let (mut gang_bodies, mut gang_stat) = (0u32, 0u32);
    for g in world.gangs() {
        for &m in world.comp::<crate::components::Gang>(g).map(|x| x.members.as_slice()).unwrap_or_default() {
            match world.comp::<Brain>(m) {
                Some(_) if world.has::<Sentence>(m) => {}
                Some(b) if b.lod == Lod::Statistical => gang_stat += 1,
                Some(_) => gang_bodies += 1,
                None => {}
            }
        }
    }
    let row = &mut world.stats.current;
    // With the budget off the columns stay zero (the M15 row).
    row.budget.tier_held = held_by_tier.iter().sum();
    row.budget.tier_held_body = held_by_tier[Lod::Full as usize] + held_by_tier[Lod::Coarse as usize];
    row.budget.gang_bodies = gang_bodies;
    row.budget.gang_stat = gang_stat;
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
    virt_snapshot(world);
    // M15 W43 snapshots (0 with the word off, W44).
    word_snapshot(world, &citizens);
    // L2 (plan L34) snapshots (0 with `[living]` off).
    living_snapshot(world, &citizens);
    // Real economy (plan E45) snapshots (0 with the market off).
    econ_snapshot(world, &citizens);
    // L2 phase 4: the kill rates by tier (0 with `[fviolence]` off).
    crate::systems::fviolence::snapshot(world);
}

/// L2 phase 1: the day's wage/dole ratio, the employed share of adults,
/// standing venues per kind, the public works, the band and the outside.
fn living_snapshot(world: &mut World, citizens: &[EntityId]) {
    let (mut adults, mut employed) = (0u32, 0u32);
    for &id in citizens {
        if world.has::<Brain>(id) && crate::systems::demography::is_adult(world, id) {
            adults += 1;
            if world.has::<Job>(id) {
                employed += 1;
            }
        }
    }
    let mut venues = [0u32; 6];
    for (i, kind) in BuildingKind::LEISURE.into_iter().enumerate() {
        venues[i] = world
            .buildings_of_kind(kind)
            .iter()
            .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
            .count() as u32;
    }
    let works = world.jobs_book.works.len() as u32;
    let works_bill: i64 =
        world.jobs_book.works.iter().filter_map(|&a| world.comp::<Job>(a)).map(|j| j.wage_per_day).sum();
    let (mult, inbound, minted) = (world.budget.upkeep_mult, world.outside.inbound, world.outside.minted);
    // L2 phase 2: the fun columns and the standing fronts.
    let (fun_mean, fun_share) = crate::systems::leisure::fun_columns(world);
    let fronts: u32 = world.gangs().iter().map(|&g| crate::systems::leisure::fronts_of(world, g).len() as u32).sum();
    let row = &mut world.stats.current;
    row.living.fun_mean = fun_mean;
    row.living.fun_satisfied_share = fun_share;
    row.living.fronts = fronts;
    row.living.wage_dole_ratio = row.flow_wages as f32 / row.flow_dole.max(1) as f32;
    row.living.employed_share = employed as f32 / adults.max(1) as f32;
    row.living.venues = venues;
    row.living.works_jobs = works;
    row.living.flow_public_works = works_bill;
    row.living.upkeep_mult = mult;
    row.living.outside_inbound = inbound;
    row.living.outside_minted = minted;
}

/// The Real economy phase 1 (plan E45): the market's columns: the
/// balance of trade and its 30-day ring, the books (appetite, today's
/// first-unit bid, the ask, units in and out), the account, the identity,
/// the city's deeds, and the per-district Dreg count (E34).
fn econ_snapshot(world: &mut World, citizens: &[EntityId]) {
    use crate::econ::TRADE_DAYS;
    use crate::outside::ExportGood;
    use crate::systems::world_market as wm;
    let book = |g: ExportGood| wm::book(world, g).cloned().unwrap_or_default();
    let (food, parts, data) = (book(ExportGood::Food), book(ExportGood::Parts), book(ExportGood::Data));
    let bid_food = wm::bid(world, ExportGood::Food, 0) as f32;
    let ask_food = wm::ask(world, ExportGood::Food);
    let world_treasury = world.outside.faction(crate::outside::WORLD_ACCOUNT).map_or(0, |f| f.treasury);
    let identity = crate::systems::econ::identity(world);
    let outbound = world.outside.outbound;
    let (no_net, tax_rate, till) =
        (crate::systems::econ::no_net(world), world.levers.tax_rate, world.econ.recycler_till);
    // Phase 2 (plan E45): the wage columns, wages on only.
    let wages = crate::systems::wages::on(world);
    let (wage_mult_mean, wage_gross_mean, vacancies_open) = if wages {
        (
            crate::systems::wages::mean_wage_rev(world),
            crate::systems::wages::gross_mean(world),
            crate::systems::wages::vacancies_open(world),
        )
    } else {
        (0.0, 0.0, 0)
    };
    let city_owned = world
        .with::<Building>()
        .into_iter()
        .filter_map(|b| world.comp::<Building>(b))
        .filter(|bd| !bd.demolished && !bd.derelict && bd.owner.is_none())
        .filter(|bd| crate::systems::ownership::role_for(bd.kind).is_some())
        .count() as u32;
    let execs = crate::systems::classes::exec_set(world);
    let mut dregs = [0u32; crate::stats::DISTRICT_SLOTS];
    for &id in citizens {
        if !world.has::<Brain>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        if crate::systems::classes::class_in(world, id, &execs) != crate::components::Class::Dreg {
            continue;
        }
        let d = match world.comp::<Household>(id).and_then(|h| h.home) {
            Some(h) => world.district_of_building(h),
            None => world
                .comp::<crate::components::Position>(id)
                .map_or(crate::components::DistrictId(0), |p| world.district_of(p.tile)),
        };
        if let Some(n) = dregs.get_mut(d.index()) {
            *n += 1;
        }
    }
    let row = &mut world.stats.current;
    let e = &mut row.econ;
    e.export_paid = row.living.flow_export;
    e.trade_balance =
        e.export_paid + e.flow_migrant_in + e.flow_fence - e.flow_import_out - e.flow_inputs - e.flow_migrant_out;
    let ring = &mut world.econ.trade_ring;
    if ring.len() >= TRADE_DAYS {
        ring.pop_front();
    }
    ring.push_back(e.trade_balance);
    e.trade_balance_30 = ring.iter().sum();
    e.appetite_food = food.appetite;
    e.appetite_parts = parts.appetite;
    e.appetite_data = data.appetite;
    e.bid_food = bid_food;
    e.ask_food = ask_food;
    e.food_imported = food.sold_today;
    e.food_exported = food.bought_today;
    e.parts_exported = parts.bought_today;
    e.data_exported = data.bought_today;
    e.outside_outbound = outbound;
    e.world_treasury = world_treasury;
    e.coin_identity = identity;
    e.city_owned_buildings = city_owned;
    e.d_dregs = dregs;
    if wages {
        e.wage_mult_mean = wage_mult_mean;
        e.wage_gross_mean = wage_gross_mean;
        e.vacancies_open = vacancies_open;
        e.pop_inflow_wages = row.flow_wages;
    }
    // Phase 3a (plan E21, E33, E45): the band's rate and the till.
    if no_net {
        e.tax_rate = tax_rate;
        e.recycler_till = till;
    }
}

/// M15 W43: the second-hand share of held deed memories (heard ÷ all), the
/// longest rumour held, the pools' summed reach, per gang slot dread and
/// heat, per corp slot honour and standing (the last midnight's rebuild).
fn word_snapshot(world: &mut World, citizens: &[EntityId]) {
    let (mut heard, mut all, mut hops) = (0u32, 0u32, 0u32);
    // M15 W43 (phase 2): adults with any social skill ≥ 0.8.
    let (mut adults, mut rare) = (0u32, 0u32);
    for &id in citizens {
        if crate::systems::demography::is_adult(world, id) {
            if let Some(s) = world.comp::<crate::components::Skills>(id) {
                adults += 1;
                if s.social_all().iter().any(|&v| v >= 0.8) {
                    rare += 1;
                }
            }
        }
        let Some(m) = world.comp::<crate::components::Memory>(id) else { continue };
        for e in &m.entries {
            if crate::systems::memory::deed_of(id, e).is_some() {
                all += 1;
            }
        }
        for e in &m.heard {
            if crate::systems::memory::deed_of(id, e).is_some() {
                all += 1;
                heard += 1;
                hops = hops.max(u32::from(e.hops));
            }
        }
    }
    let reach: f32 = world.rumours.iter().flat_map(|p| p.entries.iter()).map(|e| e.reach).sum();
    let mut gangs = vec![[0.0f32; 2]; crate::stats::GANG_SLOTS];
    for g in world.gang_list().to_vec() {
        let i = world.gang_index(g);
        if i < gangs.len() {
            let r = crate::systems::reputation::rep(world, g);
            gangs[i] = [r.dread, r.heat];
        }
    }
    let mut corps = vec![[0.0f32; 3]; crate::stats::CORP_SLOTS];
    for c in world.corps() {
        let Some(s) = world.comp::<crate::components::Corp>(c).and_then(|cc| cc.slot).map(usize::from) else {
            continue;
        };
        if s < corps.len() {
            let r = crate::systems::reputation::rep(world, c);
            let comp = world.comp::<crate::components::Corp>(c).map_or(0.0, |cc| cc.competence);
            corps[s] = [r.honour, r.standing, comp];
        }
    }
    let law_comp = world.law().map_or(0.0, |l| l.competence);
    // M15 phase 3: the Hunts under way, the longest chain among them (a
    // length: chain + 1), the open vendettas.
    let hunts_active = world.hunts.len() as u32;
    let chain_now = world.hunts.values().map(|s| u32::from(s.chain) + 1).max().unwrap_or(0);
    let vendettas_open = world.vendettas.len() as u32;
    let w = &mut world.stats.current.word;
    w.hunts_active = hunts_active;
    w.chain_max = w.chain_max.max(chain_now);
    w.vendettas_open = vendettas_open;
    w.second_hand_share = if all > 0 { heard as f32 / all as f32 } else { 0.0 };
    w.rumour_hops_max = w.rumour_hops_max.max(hops);
    w.pool_reach = reach;
    w.gangs = gangs;
    w.corps = corps;
    w.skill_rare_share = if adults > 0 { rare as f32 / adults as f32 } else { 0.0 };
    w.law_competence = law_comp;
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
    let decks = crate::systems::virt::decks_owned(world);
    // Phase 3 (V43): cameras posted and working.
    let cameras = world
        .buildings_by_kind
        .values()
        .flatten()
        .filter(|&&b| crate::systems::virt::camera_at(world, b).is_some())
        .count() as u32;
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
    v.decks = decks;
    v.cameras = cameras;
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
