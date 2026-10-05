//! Economy: daily price and spoilage, plus the wage, dole, trade, production
//! and hauling primitives the execution layer calls at action boundaries.

use crate::components::{Brain, Building, BuildingKind, Inventory, Job, Market, MemoryKind, Needs, Skills, Wallet};
use crate::config::EconomyCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time;
use crate::world::World;

/// `clamp(round(3 × sqrt(600 / max(stock, 7))), 1, 30)`.
pub fn price_for_stock(cfg: &EconomyCfg, stock: u32) -> i64 {
    let s = f64::from(stock.max(cfg.price_min_stock));
    let p = (cfg.price_base * (cfg.price_ref_stock / s).sqrt()).round() as i64;
    p.clamp(1, cfg.price_cap)
}

pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    daily_restock(world);
    daily_price(world);
    daily_spoilage(world);
}

/// Clerks top each Market up from the Warehouse while it is under
/// `restock_floor`, at most `restock_batch` a day per Market, in ascending
/// order. See config.toml for why this deviates from the spec.
fn daily_restock(world: &mut World) {
    let floor = world.config.economy.restock_floor;
    let batch = world.config.economy.restock_batch;
    let Some(wh) = world.building_of_kind(BuildingKind::Warehouse) else { return };
    // Every Market's shortfall; when the Reserve cannot cover them all it is
    // split in proportion to the shortfall (floor division, the remainder one
    // unit at a time to the lowest ids), so no Market starves last-in-line.
    let wants: Vec<(EntityId, u32, u32)> = world
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .filter_map(|&mk| {
            let b = world.comp::<Building>(mk).filter(|b| !b.demolished)?;
            Some((mk, b.stock_food, floor.saturating_sub(b.stock_food).min(batch)))
        })
        .collect();
    let available = world.comp::<Building>(wh).map_or(0, |b| b.stock_food);
    let total: u32 = wants.iter().map(|w| w.2).sum();
    let mut shares: Vec<u32> = if total <= available {
        wants.iter().map(|w| w.2).collect()
    } else {
        wants.iter().map(|w| (u64::from(w.2) * u64::from(available) / u64::from(total.max(1))) as u32).collect()
    };
    if total > available {
        let mut left = available - shares.iter().sum::<u32>();
        for (share, w) in shares.iter_mut().zip(&wants) {
            if left == 0 {
                break;
            }
            if *share < w.2 {
                *share += 1;
                left -= 1;
            }
        }
    }
    for (&(mk, stock, _), &moved) in wants.iter().zip(&shares) {
        if moved == 0 {
            continue;
        }
        let available = world.comp::<Building>(wh).map_or(0, |b| b.stock_food);
        if let Some(b) = world.comp_mut::<Building>(wh) {
            b.stock_food -= moved;
        }
        if let Some(b) = world.comp_mut::<Building>(mk) {
            b.stock_food += moved;
        }
        world.push_event(
            EventKind::Restock,
            &[mk],
            format!(
                "Clerks restocked {moved} food from the Reserve Depot (Street Market#{} {stock} -> {}, reserve {})",
                mk.index,
                stock + moved,
                available - moved
            ),
        );
    }
}

/// Each Market prices from its own stock and keeps its own history.
fn daily_price(world: &mut World) {
    world.mean_price_cache = None;
    for market_id in world.buildings_of_kind(BuildingKind::Market).to_vec() {
        let stock = world.comp::<Building>(market_id).map_or(0, |b| b.stock_food);
        let price = price_for_stock(&world.config.economy, stock);
        let Some(m) = world.comp_mut::<Market>(market_id) else { continue };
        let old = m.price_food;
        m.price_food = price;
        if m.price_history.len() >= 120 {
            m.price_history.pop_front();
        }
        m.price_history.push_back(price);
        if price != old {
            world.push_event(
                EventKind::PriceChange,
                &[market_id],
                format!("Food price {old} -> {price} (Street Market#{} stock {stock})", market_id.index),
            );
        }
    }
}

fn daily_spoilage(world: &mut World) {
    let pantry = world.config.economy.spoilage_pantry;
    let market = world.config.economy.spoilage_market;
    for id in world.with::<Building>() {
        let Some(b) = world.comp_mut::<Building>(id) else { continue };
        let rate = match b.kind {
            BuildingKind::Home => pantry,
            BuildingKind::Market | BuildingKind::Warehouse => market,
            _ => continue,
        };
        b.stock_food -= (b.stock_food as f32 * rate).floor() as u32;
    }
    for id in world.citizens() {
        let Some(inv) = world.comp_mut::<Inventory>(id) else { continue };
        let lost = (inv.food as f32 * pantry).floor() as u32;
        inv.food -= lost;
        inv.stolen_food = inv.stolen_food.min(inv.food);
    }
}

/// Farm yield for `ticks` of work by `farmer` at `farm`: whole units move into
/// the farm's stock, the remainder stays in `production_accum`.
pub fn accrue_farm_work(world: &mut World, farmer: EntityId, farm: EntityId, ticks: u64) {
    let cfg = &world.config.economy;
    let farming = world.comp::<Skills>(farmer).map_or(0.0, |s| s.farming);
    let mult = cfg.season_mult[world.season().index()];
    let per_hour = cfg.farm_yield_base * (cfg.farm_skill_floor + cfg.farm_skill_slope * farming) * mult;
    // Fractional hours: whole units leave the accumulator, the rest carries, so
    // a shift that starts a few ticks late loses a few ticks, not an hour.
    let hours = ticks as f32 / time::TICKS_PER_HOUR as f32;
    let cap = world.config.buildings.farm.stock_cap;
    if let Some(b) = world.comp_mut::<Building>(farm) {
        b.production_accum += per_hour * hours;
        let whole = b.production_accum.floor();
        b.production_accum -= whole;
        b.stock_food = (b.stock_food + whole as u32).min(cap);
    }
}

/// Move `min(haul_batch, stock)` food from a farm to the Market nearest its
/// door, overflowing to the Warehouse and then lost. Returns units moved out
/// of the farm.
pub fn haul(world: &mut World, farm: EntityId) -> u32 {
    let batch = world.config.economy.haul_batch;
    let market_cap = world.config.buildings.market.stock_cap;
    let wh_cap = world.config.buildings.warehouse.stock_cap;
    let market = world.comp::<Building>(farm).and_then(|b| world.nearest_of_kind(BuildingKind::Market, b.door));
    let Some(b) = world.comp_mut::<Building>(farm) else { return 0 };
    let moved = batch.min(b.stock_food);
    b.stock_food -= moved;
    let mut left = moved;
    if let Some(m) = market.and_then(|m| world.comp_mut::<Building>(m)) {
        let take = left.min(market_cap.saturating_sub(m.stock_food));
        m.stock_food += take;
        left -= take;
    }
    if left > 0 {
        if let Some(w) = world.building_of_kind(BuildingKind::Warehouse).and_then(|w| world.comp_mut::<Building>(w)) {
            let take = left.min(wh_cap.saturating_sub(w.stock_food));
            w.stock_food += take;
        }
    }
    moved
}

/// Units `agent` can afford, carry and `market` can supply right now:
/// `min(3, floor(coins / price), 20 - food, Market stock)`.
pub fn buy_quantity(world: &World, agent: EntityId, market: Option<EntityId>) -> u32 {
    let Some(market) = market else { return 0 };
    let price = world.comp::<Market>(market).map_or(i64::MAX, |m| m.price_food).max(1);
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    let food = world.comp::<Inventory>(agent).map_or(20, |i| i.food);
    let stock = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
    let afford = (coins / price).clamp(0, 3) as u32;
    afford.min(20u32.saturating_sub(food)).min(stock)
}

/// Pay for `units` at `market`'s price (into the Treasury). Returns the coins paid.
pub fn pay_for_food(world: &mut World, agent: EntityId, market: Option<EntityId>, units: u32) -> i64 {
    let price = market.and_then(|m| world.comp::<Market>(m)).map_or(0, |m| m.price_food);
    let cost = price * i64::from(units);
    if let Some(w) = world.comp_mut::<Wallet>(agent) {
        w.coins -= cost;
    }
    if let Some(t) = world.treasury_mut() {
        t.coins += cost;
    }
    cost
}

/// Take `units` off `market`'s shelf into the agent's inventory. Returns false
/// (and refunds) if the stock is gone.
pub fn take_food(world: &mut World, agent: EntityId, market: Option<EntityId>, units: u32, paid: i64) -> bool {
    let stock = market.and_then(|m| world.comp::<Building>(m)).map_or(0, |b| b.stock_food);
    if stock < units || units == 0 {
        if let Some(w) = world.comp_mut::<Wallet>(agent) {
            w.coins += paid;
        }
        if let Some(t) = world.treasury_mut() {
            t.coins -= paid;
        }
        return false;
    }
    if let Some(b) = market.and_then(|m| world.comp_mut::<Building>(m)) {
        b.stock_food -= units;
    }
    if let Some(i) = world.comp_mut::<Inventory>(agent) {
        i.food += units;
    }
    world.remember(agent, MemoryKind::Paid, None, 0.1, 0.0, false);
    true
}

/// `CollectWage` at the Hall, once per day whatever the outcome. Returns the net coins paid.
pub fn collect_wage(world: &mut World, agent: EntityId) -> i64 {
    let tax_rate = world.levers.tax_rate;
    let day = world.day();
    let Some(job) = world.comp::<Job>(agent).cloned() else { return 0 };
    if job.days_unpaid == 0 {
        return 0;
    }
    let due = job.wage_per_day * i64::from(job.days_unpaid);
    let mut tax_accum = job.tax_accum + due as f32 * tax_rate;
    let tax = tax_accum.floor() as i64;
    tax_accum -= tax as f32;
    let net = due - tax;
    let available = world.treasury().map_or(0, |t| t.coins).max(0);
    let paid = net.min(available);
    if let Some(t) = world.treasury_mut() {
        t.coins -= paid;
    }
    if let Some(w) = world.comp_mut::<Wallet>(agent) {
        w.coins += paid;
    }
    let remainder = net - paid;
    let days_left = if remainder > 0 { ((remainder + job.wage_per_day - 1) / job.wage_per_day) as u8 } else { 0 };
    if let Some(j) = world.comp_mut::<Job>(agent) {
        j.tax_accum = tax_accum;
        j.days_unpaid = days_left;
    }
    if remainder == 0 {
        world.remember(agent, MemoryKind::Paid, None, 0.3, 0.3, false);
        crate::systems::social::repay_debts(world, agent);
    } else {
        // Short: no second visit today.
        if let Some(j) = world.comp_mut::<Job>(agent) {
            j.last_wage_attempt_day = Some(day);
        }
        world.remember(agent, MemoryKind::Unpaid, None, 0.5, -0.5, false);
        if let Some(n) = world.comp_mut::<Needs>(agent) {
            n.wealth = (n.wealth - 0.1).max(0.0);
        }
        if let Some(p) = world.comp_mut::<crate::components::Personality>(agent) {
            p.lawfulness = (p.lawfulness - 0.01).max(0.0);
            p.loyalty = (p.loyalty - 0.02).max(0.0);
        }
    }
    maybe_quit(world, agent);
    paid
}

/// A worker with `days_unpaid >= 7`, or three Unpaid memories in 7 days, quits.
pub fn maybe_quit(world: &mut World, agent: EntityId) {
    let Some(job) = world.comp::<Job>(agent) else { return };
    let week_ago = world.tick.saturating_sub(7 * time::TICKS_PER_DAY);
    let unpaid_memories = world
        .comp::<crate::components::Memory>(agent)
        .map_or(0, |m| m.entries.iter().filter(|e| e.kind == MemoryKind::Unpaid && e.tick >= week_ago).count());
    if job.days_unpaid >= 7 || unpaid_memories >= 3 {
        quit_job(world, agent, "unpaid");
    }
}

/// Remove the Job (posting a vacancy), drop the plan, log the event.
pub fn quit_job(world: &mut World, agent: EntityId, reason: &str) {
    let Some(job) = world.vacate_job(agent) else { return };
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        b.clear_plan();
    }
    let name = world.name_of(agent);
    world.push_event(EventKind::Quit, &[agent], format!("{name} quit as {} ({reason})", job.role.label()));
}

/// `CollectDole` at the Hall, once per day, while the Treasury is not negative.
pub fn collect_dole(world: &mut World, agent: EntityId) -> bool {
    let day = world.day();
    let dole = i64::from(world.levers.dole_per_day);
    let treasury = world.treasury().map_or(0, |t| t.coins);
    let already = world.comp::<Brain>(agent).is_some_and(|b| b.last_dole_day == Some(day));
    if already || treasury < 0 || dole <= 0 || world.has::<Job>(agent) {
        return false;
    }
    if let Some(t) = world.treasury_mut() {
        t.coins -= dole;
    }
    if let Some(w) = world.comp_mut::<Wallet>(agent) {
        w.coins += dole;
    }
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        b.last_dole_day = Some(day);
    }
    true
}
