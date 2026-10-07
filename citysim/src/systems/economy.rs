//! Economy: daily price and spoilage, plus the wage, dole, trade, production
//! and hauling primitives the execution layer calls at action boundaries.

use crate::components::{
    Brain, Building, BuildingKind, Corp, Inventory, Job, Market, MemoryKind, Needs, Niche, Skills, Wallet,
};
use crate::config::EconomyCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
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
    // M13 D40: legal Stims on the Markets' shelves (the lever on only).
    crate::systems::stims::roll_sales(world);
    crate::systems::stims::restock_legal(world);
    daily_price(world);
    daily_spoilage(world);
}

/// Clerks top each Market up from the Warehouse while it is under
/// `restock_floor`, at most `restock_batch` a day per Market, in ascending
/// order. See config.toml for why this deviates from the spec. M11 D5: a
/// Market the city does not own buys its restock from the city at
/// `[corps] wholesale` a unit, as much as its owner's budget covers. Phase
/// 5: a corp's budget is its closing balance before tonight's upkeep (the
/// restock runs after the upkeep lump, and a corp with a day's profit but
/// less than a day's upkeep in the bank left its shelf empty, price 30),
/// shared across its Markets; the restock is charged in full.
fn daily_restock(world: &mut World) {
    let floor = world.config.economy.restock_floor;
    let batch = world.config.economy.restock_batch;
    let wholesale = world.config.corps.wholesale;
    let Some(wh) = world.building_of_kind(BuildingKind::Warehouse) else { return };
    // Every Market's shortfall; when the Reserve cannot cover them all it is
    // split in proportion to the shortfall (floor division, the remainder one
    // unit at a time to the lowest ids), so no Market starves last-in-line.
    let mut budget: std::collections::BTreeMap<EntityId, i64> = std::collections::BTreeMap::new();
    let mut wants: Vec<(EntityId, u32, u32)> = Vec::new();
    for &mk in world.buildings_of_kind(BuildingKind::Market) {
        // Fix pass (phase 4 review): a Market a riot closed takes no delivery.
        let Some(b) = world.comp::<Building>(mk).filter(|b| !b.demolished && !world.is_closed(mk)) else { continue };
        let mut want = floor.saturating_sub(b.stock_food).min(batch);
        if let Some(o) = b.owner.filter(|_| wholesale > 0) {
            let left = budget.entry(o).or_insert_with(|| {
                let purse = world.purse(Some(o));
                world.comp::<Corp>(o).map_or(purse, |c| c.closing.max(purse))
            });
            let afford = ((*left).max(0) / wholesale).min(i64::from(u32::MAX)) as u32;
            want = want.min(afford);
            *left -= i64::from(want) * wholesale;
        }
        wants.push((mk, b.stock_food, want));
    }
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
        let owner = world.owner_of(mk);
        if owner.is_some() {
            let paid = ownership::charge(world, owner, None, i64::from(moved) * wholesale, Flow::Wholesale);
            world.stats.current.flow_restock += paid;
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

/// Each Market prices from its own stock and keeps its own history. M11: a
/// corp's Food `price_level` marks the price up or down; the day's sales and
/// opening stock roll into the seven-day windows (D16).
fn daily_price(world: &mut World) {
    world.mean_price_cache = None;
    let cap = world.config.economy.price_cap;
    for market_id in world.buildings_of_kind(BuildingKind::Market).to_vec() {
        let stock = world.comp::<Building>(market_id).map_or(0, |b| b.stock_food);
        let level =
            world.corp_of_building(market_id).and_then(|c| world.comp::<Corp>(c)).map_or(1.0, |c| c.level(Niche::Food));
        let base = price_for_stock(&world.config.economy, stock);
        let price = if level == 1.0 { base } else { ((base as f32 * level).round() as i64).clamp(1, cap) };
        // Phase 5: the fraction a whole coin hides (0.9 x 3 = 2.7, not 3).
        let tenths = if world.config.economy.price_tenths && level != 1.0 {
            ((base as f32 * level * 10.0).round() as i64).clamp(10, cap * 10)
        } else {
            price * 10
        };
        let Some(m) = world.comp_mut::<Market>(market_id) else { continue };
        let old = m.price_food;
        m.price_food = price;
        m.price_tenths = tenths;
        if m.price_history.len() >= 120 {
            m.price_history.pop_front();
        }
        m.price_history.push_back(price);
        let sold = std::mem::take(&mut m.sales_today);
        m.sales.push_back(sold);
        m.stock_hist.push_back(stock);
        while m.sales.len() > 7 {
            m.sales.pop_front();
        }
        while m.stock_hist.len() > 7 {
            m.stock_hist.pop_front();
        }
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
    let sump = world.config.rent.sump_spoilage_mult;
    for id in world.with::<Building>() {
        let Some(b) = world.comp_mut::<Building>(id) else { continue };
        let rate = match b.kind {
            // M11 section 4: Sump pantries spoil faster.
            BuildingKind::Home if b.tier == 0 => (pantry * sump).min(1.0),
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
    let mut per_hour = cfg.farm_yield_base * (cfg.farm_skill_floor + cfg.farm_skill_slope * farming) * mult;
    // M14 (spec § 5): a corp-owned Farm yields `x (1 + industry_prod[tier])`
    // of its owner's Industry tier (a branch: tier 1 multiplies nothing).
    if world.config.virt.enabled {
        let tier = world
            .corp_of_building(farm)
            .and_then(|c| world.comp::<crate::components::Corp>(c))
            .map_or(1, |c| c.tech.tier_of(crate::virt::Track::Industry));
        if tier >= 2 {
            let p = world.config.tech.industry_prod.get(usize::from(tier)).copied().unwrap_or(0.0);
            per_hour *= 1.0 + p;
        }
    }
    // M15 W28: a corp's Farm yields × its competence multiplier (1 when off).
    if let Some(c) = world.corp_of_building(farm) {
        let m = crate::systems::competence::comp_mult(world, c);
        if m != 1.0 {
            per_hour *= m;
        }
    }
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

/// Move `min(haul_batch, stock)` food from a farm to its owner's nearest
/// Market (any Market's nearest when the owner has none), overflowing to the
/// Warehouse and then lost. Returns units moved out of the farm. M11 D5:
/// between different owners the Market's owner buys at `[corps] wholesale`
/// a unit (units it cannot afford go to the Reserve), and the city buys the
/// Reserve's share from a non-city Farm at the same price. M13 D23: a
/// vehicle's haul is `haul_batch × mult` (`Some(mult)`; `None` on foot),
/// counted as a truck haul (a car's too) or a walked one.
pub fn haul(world: &mut World, farm: EntityId, vehicle: Option<u32>) -> u32 {
    let batch = world.config.economy.haul_batch * vehicle.unwrap_or(1).max(1);
    if world.config.assets.enabled {
        match vehicle {
            Some(_) => world.stats.current.truck_hauls += 1,
            None => world.stats.current.walk_hauls += 1,
        }
    }
    let market_cap = world.config.buildings.market.stock_cap;
    let wh_cap = world.config.buildings.warehouse.stock_cap;
    let wholesale = world.config.corps.wholesale;
    let Some((farm_door, farm_owner)) = world.comp::<Building>(farm).map(|b| (b.door, b.owner)) else { return 0 };
    // Fix pass (phase 4 review): no haul into a Market a riot closed (the
    // batch goes to the Warehouse as an overflow would).
    let market = ownership::owned_of_kind(world, farm_owner, BuildingKind::Market)
        .into_iter()
        .filter(|&m| !world.is_closed(m))
        .filter_map(|m| world.comp::<Building>(m).map(|b| (b.door.manhattan(farm_door), m)))
        .min()
        .map(|(_, m)| m)
        .or_else(|| world.nearest_of_kind(BuildingKind::Market, farm_door))
        .filter(|&m| !world.is_closed(m));
    let Some(b) = world.comp_mut::<Building>(farm) else { return 0 };
    let moved = batch.min(b.stock_food);
    b.stock_food -= moved;
    let mut left = moved;
    if let Some(m) = market {
        let (room, market_owner) =
            world.comp::<Building>(m).map_or((0, None), |mb| (market_cap.saturating_sub(mb.stock_food), mb.owner));
        let mut take = left.min(room);
        if market_owner != farm_owner && wholesale > 0 {
            let afford = (world.purse(market_owner).max(0) / wholesale).min(i64::from(u32::MAX)) as u32;
            take = take.min(afford);
            let paid = ownership::pay(world, market_owner, farm_owner, i64::from(take) * wholesale, Flow::Wholesale);
            ownership::credit(world, farm, paid);
        }
        if let Some(mb) = world.comp_mut::<Building>(m) {
            mb.stock_food += take;
        }
        left -= take;
    }
    if left > 0 {
        if let Some(w) = world.building_of_kind(BuildingKind::Warehouse) {
            let room = world.comp::<Building>(w).map_or(0, |wb| wh_cap.saturating_sub(wb.stock_food));
            let take = left.min(room);
            if let Some(wb) = world.comp_mut::<Building>(w) {
                wb.stock_food += take;
            }
            if farm_owner.is_some() && wholesale > 0 {
                let paid = ownership::charge(world, None, farm_owner, i64::from(take) * wholesale, Flow::Wholesale);
                ownership::credit(world, farm, paid);
                world.stats.current.flow_overflow += paid;
            }
        }
    }
    moved
}

/// Units `agent` can afford, carry and `market` can supply right now:
/// `min(3, floor(coins / price), FOOD_CAP - food, capacity - load, Market
/// stock)` (M13 D7). With no stims or parts `capacity - load >= 20 - food`
/// (capacity is at least 22 by the defaults, 20 with assets off), so every
/// purchase without them is the M12 one.
pub fn buy_quantity(world: &World, agent: EntityId, market: Option<EntityId>) -> u32 {
    use crate::systems::assets::{capacity, load, FOOD_CAP};
    let Some(market) = market else { return 0 };
    let price = if world.has::<Market>(market) { world.price_for(market, agent) } else { i64::MAX };
    let coins = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
    let inv = world.comp::<Inventory>(agent);
    let food = inv.map_or(FOOD_CAP, |i| i.food);
    let room = inv.map_or(0, |i| capacity(world, agent).saturating_sub(load(i)));
    let stock = world.comp::<Building>(market).map_or(0, |b| b.stock_food);
    let afford = (coins / price).clamp(0, 3) as u32;
    afford.min(FOOD_CAP.saturating_sub(food)).min(room).min(stock)
}

/// Pay for `units` at `market`'s price to the Market's owner (M11; the
/// Treasury before, which still owns a city Market). Returns the coins paid.
pub fn pay_for_food(world: &mut World, agent: EntityId, market: Option<EntityId>, units: u32) -> i64 {
    let price = market.filter(|&m| world.has::<Market>(m)).map_or(0, |m| world.price_for(m, agent));
    let cost = price * i64::from(units);
    let owner = market.and_then(|m| world.owner_of(m));
    let paid = ownership::pay(world, Some(agent), owner, cost, Flow::Food);
    if let Some(m) = market {
        ownership::credit(world, m, paid);
    }
    paid
}

/// Return a purchase whose stock was gone (`pay_for_food` reversed).
pub fn refund_food(world: &mut World, agent: EntityId, market: Option<EntityId>, paid: i64) {
    let owner = market.and_then(|m| world.owner_of(m));
    ownership::refund(world, agent, owner, paid, Flow::Food);
    if let Some(m) = market {
        ownership::credit(world, m, -paid);
    }
}

/// Take `units` off `market`'s shelf into the agent's inventory. Returns false
/// (and refunds) if the stock is gone.
pub fn take_food(world: &mut World, agent: EntityId, market: Option<EntityId>, units: u32, paid: i64) -> bool {
    let stock = market.and_then(|m| world.comp::<Building>(m)).map_or(0, |b| b.stock_food);
    if stock < units || units == 0 {
        refund_food(world, agent, market, paid);
        return false;
    }
    if let Some(b) = market.and_then(|m| world.comp_mut::<Building>(m)) {
        b.stock_food -= units;
    }
    if let Some(m) = market.and_then(|m| world.comp_mut::<Market>(m)) {
        m.sales_today += units;
    }
    if let Some(i) = world.comp_mut::<Inventory>(agent) {
        i.food += units;
    }
    world.remember(agent, MemoryKind::Paid, None, 0.1, 0.0, false);
    true
}

/// `CollectWage` at the wage desk (the Hall for a city job, else the
/// workplace: M11 D13), once per day whatever the outcome. The employer
/// building's owner pays; nobody pays from a negative purse. Returns the net
/// coins paid.
pub fn collect_wage(world: &mut World, agent: EntityId) -> i64 {
    let tax_rate = world.levers.tax_rate;
    let day = world.day();
    let Some(job) = world.comp::<Job>(agent).cloned() else { return 0 };
    if job.days_unpaid == 0 {
        return 0;
    }
    let payer = job.employer.and_then(|e| world.owner_of(e));
    // D22: a Food corp in Squeeze pays 0.9.
    // M15 W29: a poached hire's premium multiplies too.
    let mult = payer.and_then(|p| world.comp::<Corp>(p)).map_or(1.0, |c| c.wage_mult) * job.premium;
    let per_day = if mult == 1.0 { job.wage_per_day } else { (job.wage_per_day as f32 * mult).round() as i64 };
    let due = per_day * i64::from(job.days_unpaid);
    let available = world.purse(payer).max(0);
    // The gross a payer can cover: a non-city employer pays the net wage and
    // the withheld tax out of the same purse, so both are capped together
    // and tax accrues only on what is paid (it was capped after the wage and
    // the Treasury shorted while `tax_accum` counted it). The city's own
    // wage tax never leaves it: it pays the net.
    let (gross, mut tax_accum, tax) = if payer.is_some() {
        let gross = due.min(available);
        let acc = job.tax_accum + gross as f32 * tax_rate;
        let tax = (acc.floor() as i64).clamp(0, gross);
        (gross, acc - tax as f32, tax)
    } else {
        let acc = job.tax_accum + due as f32 * tax_rate;
        let tax = acc.floor() as i64;
        let gross = (due - tax).min(available) + tax;
        (gross, acc - tax as f32, tax)
    };
    let paid = gross - tax;
    ownership::pay(world, payer, Some(agent), paid, Flow::Wage);
    if payer.is_some() && tax > 0 {
        ownership::pay(world, payer, None, tax, Flow::Tax);
    }
    let remainder = due - gross;
    let days_left = if remainder > 0 { ((remainder + per_day.max(1) - 1) / per_day.max(1)) as u8 } else { 0 };
    tax_accum = tax_accum.max(0.0);
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
    if paid > 0 {
        ownership::pay_rent_from_income(world, agent);
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

/// An employer lets `agent` go: no vacancy is posted (a layoff, a dismissal
/// or a closed office is a job that no longer exists). The plan is aborted
/// first, so an escort in progress ends and the suspect's `cuffed_by` is
/// cleared, a running Use is settled, and the Job (with its `duty_ticks`)
/// goes. `Fire` event with `text`, the building in slot 1 when given.
pub fn dismiss(world: &mut World, agent: EntityId, building: Option<EntityId>, text: String) -> Option<Job> {
    world.abort_plan(agent);
    let job = world.remove::<Job>(agent)?;
    let mut actors = vec![agent];
    actors.extend(building);
    world.push_event(EventKind::Fire, &actors, text);
    Some(job)
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
    world.stats.current.flow_dole += dole;
    if let Some(b) = world.comp_mut::<Brain>(agent) {
        b.last_dole_day = Some(day);
    }
    ownership::pay_rent_from_income(world, agent);
    true
}
