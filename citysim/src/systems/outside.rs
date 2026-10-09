//! Life pass L2 § 1 (plan L11): the export hook. Daily at midnight, after
//! the budget band: with export on, the World account (created at
//! `outside::WORLD_ACCOUNT` if absent) is refilled to its `treasury_ref`
//! (`minted += refill`) and buys up to `cap_per_day` of each good above its
//! floor at `price × market`: Food from Farms, Parts from Fabs, Data from
//! Labs' stores. Each sale crosses in (`ownership::cross_in`,
//! `Flow::Export`) and the owner pays the sale's tax to the Treasury. With
//! export off nothing runs and `World::outside` stays empty.

use crate::components::{Building, BuildingKind, Good};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{ExportGood, OutsideFaction, OutsideKind, WORLD_ACCOUNT};
use crate::systems::ownership::{self, Flow};
use crate::world::World;

/// `[living] enabled` and the World account's buying open (`levers.export_open`,
/// set from `[export] enabled` at seed; `SetExport` toggles it).
pub fn export_on(world: &World) -> bool {
    world.config.living.enabled && world.levers.export_open
}

/// The World account, created on first use (Real economy plan E4: the
/// market seeds it with `[world_market] treasury_ref`; L2's export hook
/// with `[export] treasury_ref`).
pub(crate) fn ensure_world_account(world: &mut World, treasury_ref: i64) {
    if world.outside.faction(WORLD_ACCOUNT).is_some() {
        return;
    }
    world.outside.factions.push(OutsideFaction {
        id: WORLD_ACCOUNT,
        name: "the World".to_string(),
        kind: OutsideKind::World,
        treasury: 0,
        treasury_ref,
        base_income: 0,
        base_upkeep: 0,
        income_today: 0,
        market: 1.0,
        dead: false,
        books: Default::default(),
    });
    world.outside.next_id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
}

/// Today's price per unit of a good (a god's `SetExportPrice` first).
pub fn price_of(world: &World, good: ExportGood) -> i64 {
    if let Some(&p) = world.outside.export.price.get(&good) {
        return p;
    }
    let p = &world.config.export.price;
    match good {
        ExportGood::Food => p.food,
        ExportGood::Parts => p.parts,
        ExportGood::Data => p.data,
    }
}

fn cap_of(world: &World, good: ExportGood) -> i64 {
    let c = &world.config.export.cap_per_day;
    match good {
        ExportGood::Food => c.food,
        ExportGood::Parts => c.parts,
        ExportGood::Data => c.data,
    }
}

/// Pay `units` of `good` at today's price to `owner`, as far as the World
/// account's treasury goes; returns the units paid for and the coins.
fn sell(world: &mut World, owner: Option<EntityId>, good: ExportGood, units: u32) -> (u32, i64) {
    let market = world.outside.faction(WORLD_ACCOUNT).map_or(1.0, |f| f.market);
    let unit = ((price_of(world, good) as f32 * market).round() as i64).max(0);
    if unit == 0 || units == 0 {
        return (0, 0);
    }
    let purse = world.outside.faction(WORLD_ACCOUNT).map_or(0, |f| f.treasury.max(0));
    let units = units.min(u32::try_from(purse / unit).unwrap_or(u32::MAX));
    if units == 0 {
        return (0, 0);
    }
    let paid = ownership::cross_in(world, WORLD_ACCOUNT, owner, i64::from(units) * unit, Flow::Export);
    // The sale's tax, explicit (the crossing is untaxed, L11).
    let tax = (world.levers.tax_rate * paid as f32).round() as i64;
    if owner.is_some() && tax > 0 {
        ownership::pay(world, owner, None, tax, Flow::Tax);
    }
    *world.outside.export.sold.entry(good).or_default() += u64::from(units);
    *world.outside.export.paid.entry(good).or_default() += paid;
    (units, paid)
}

/// The midnight pass (plan L11).
pub fn export_daily(world: &mut World) {
    // Real economy (plan E9, E10): with the market on the book's pass
    // (`world_market::daily`) is the one World path.
    if !export_on(world) || crate::systems::econ::market_on(world) {
        return;
    }
    let treasury_ref = world.config.export.treasury_ref;
    ensure_world_account(world, treasury_ref);
    if let Some(f) = world.outside.faction_mut(WORLD_ACCOUNT) {
        let refill = (f.treasury_ref - f.treasury).max(0);
        f.treasury += refill;
        f.income_today = 0;
        world.outside.minted += refill;
    }
    let cfg = world.config.export.clone();
    let mut totals = [(0u32, 0i64); 3];
    // Food: Farms' stock above the floor, ascending.
    let mut left = cap_of(world, ExportGood::Food).max(0) as u32;
    for b in world.buildings_of_kind(BuildingKind::Farm).to_vec() {
        if left == 0 {
            break;
        }
        let Some((stock, owner)) =
            world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.stock_food, bd.owner))
        else {
            continue;
        };
        let units = stock.saturating_sub(cfg.export_floor).min(left);
        let (n, paid) = sell(world, owner, ExportGood::Food, units);
        world.take_stock(b, Good::Food, n);
        left -= n;
        totals[0].0 += n;
        totals[0].1 += paid;
    }
    // Parts: Fabs' stock above the floor.
    let mut left = cap_of(world, ExportGood::Parts).max(0) as u32;
    for b in world.buildings_of_kind(BuildingKind::Fab).to_vec() {
        if left == 0 {
            break;
        }
        let owner = world.owner_of(b);
        let units = world.stock(b, Good::Parts).saturating_sub(cfg.parts_floor).min(left);
        let (n, paid) = sell(world, owner, ExportGood::Parts, units);
        world.take_stock(b, Good::Parts, n);
        left -= n;
        totals[1].0 += n;
        totals[1].1 += paid;
    }
    // Data: Labs' stores above the floor, from the fullest track.
    let mut left = cap_of(world, ExportGood::Data).max(0) as u32;
    for b in world.buildings_of_kind(BuildingKind::Lab).to_vec() {
        if left == 0 {
            break;
        }
        let Some(n) = crate::systems::virt::node_of_building(world, b) else { continue };
        let Some(store) = world.virt.node(n).map(|x| x.store.clone()) else { continue };
        let units = store.total().saturating_sub(cfg.data_floor).min(left);
        let owner = world.owner_of(b);
        let (sold, paid) = sell(world, owner, ExportGood::Data, units);
        let mut take = sold;
        if let Some(node) = world.virt.node_mut(n) {
            while take > 0 {
                let Some(i) = (0..3).max_by_key(|&i| (node.store.units[i], std::cmp::Reverse(i))) else { break };
                let t = node.store.units[i].min(take);
                if t == 0 {
                    break;
                }
                node.store.units[i] -= t;
                take -= t;
            }
        }
        left -= sold;
        totals[2].0 += sold;
        totals[2].1 += paid;
    }
    let all: i64 = totals.iter().map(|t| t.1).sum();
    if all > 0 {
        let text =
            format!("the World bought {} food, {} Parts, {} Data for {all}", totals[0].0, totals[1].0, totals[2].0);
        world.push_event(EventKind::Exported, &[], text);
    }
}
