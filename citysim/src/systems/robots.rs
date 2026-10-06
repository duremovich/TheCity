//! M13 phase 4: the security robot (docs/M13_ASSETS.md § 5, plan D41, D42).
//!
//! A `Robot` asset `Posted` at a building: a defender at the front of every
//! brawl at its door, and a sensor that contests a theft or a shakedown
//! inside and detains the loser. It has no Brain, Needs, Memory or LOD: in
//! a fight it is a fighter at `robot_fighting[tier − 1]` with courage 1.0,
//! and a robot that loses is wrecked (Parts at the door), never killed.
//! Its upkeep is its power: unpaid, it neither fights nor senses.
//!
//! Every contest here is a dice roll between fictional parties. Nothing
//! runs per tick: the sensor is called by `law::raise_crime`, the defender
//! lists by the brawls, and `Secure`'s purchase by the corp's daily act.

use crate::components::{Asset, AssetKind, AssetLoc, Brain, Building, BuildingKind, Corp, Crime, Niche, ShopPick};
use crate::entity::EntityId;
use crate::systems::assets;
use crate::world::World;

/// An entity that is a robot asset (posted or not).
pub fn is_robot(world: &World, id: EntityId) -> bool {
    world.comp::<Asset>(id).is_some_and(|x| x.kind == AssetKind::Robot)
}

/// A robot posted at `b` (powered or not, not wrecked), the lowest id.
pub fn posted_robot(world: &World, b: EntityId) -> Option<EntityId> {
    assets::assets_at(world, b).iter().copied().find(|&a| {
        world
            .comp::<Asset>(a)
            .is_some_and(|x| x.kind == AssetKind::Robot && x.loc == AssetLoc::Posted(b) && x.condition > 0)
    })
}

/// D41: the powered robot posted at `b` (condition > 0, upkeep paid, not
/// bricked), the lowest id.
pub fn powered_robot(world: &World, b: EntityId) -> Option<EntityId> {
    if !world.config.assets.enabled {
        return None;
    }
    assets::assets_at(world, b).iter().copied().find(|&a| {
        world.comp::<Asset>(a).is_some_and(|x| {
            x.kind == AssetKind::Robot
                && x.loc == AssetLoc::Posted(b)
                && x.condition > 0
                && x.upkeep_arrears == 0
                && !x.bricked
        })
    })
}

/// D41: a robot's fighting, `robot_fighting[tier − 1]` (the last entry past it).
pub fn fighting(world: &World, robot: EntityId) -> Option<f32> {
    let x = world.comp::<Asset>(robot).filter(|x| x.kind == AssetKind::Robot)?;
    let table = &world.config.robots.robot_fighting;
    let i = usize::from(x.tier.saturating_sub(1)).min(table.len().saturating_sub(1));
    Some(table.get(i).copied().unwrap_or(0.5))
}

/// D41: the powered robots posted at `b`, to stand first among its defenders.
pub fn defenders_at(world: &World, b: EntityId) -> Vec<EntityId> {
    powered_robot(world, b).into_iter().collect()
}

/// D42: the tier `Secure` buys for a building of `kind`
/// (`robot_tier_by_kind`; a kind the table leaves out: 1).
pub fn robot_tier_for(world: &World, kind: BuildingKind) -> u8 {
    let t = &world.config.robots.robot_tier_by_kind;
    let tier = match kind {
        BuildingKind::Farm => t.farm,
        BuildingKind::Market => t.market,
        BuildingKind::Home => t.home,
        BuildingKind::SecurityOffice => t.security_office,
        BuildingKind::Clinic => t.clinic,
        BuildingKind::Garage => t.garage,
        _ => 1,
    };
    tier.clamp(1, 3)
}

/// What a robot's sensor made of a crime ([`sense`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensed {
    /// No powered robot, the thief beat the sensor, or the robot lost the
    /// fight (reported when it saw the crime) or could not book the thief.
    Unseen,
    /// The thief is booked (sentenced at once).
    Detained,
    /// The thief died in the fight to detain it: no report stands against
    /// a corpse, and the caller raises nothing more.
    Died,
}

/// D41, from `law::raise_crime` before the witness roll: a Theft or a
/// Shakedown inside a building with a powered robot rolls
/// `security::contest(thief tier, robot tier)` (`security::thief_tier` of
/// `law::stealth`). The robot wins: a fight to detain, and a report with no
/// witness unless the thief died in it (the dead cannot be charged); it
/// wins that fight too and the thief (alive) is booked at once
/// (`law::sentence`, as M12 books a Statistical vagrant: no cuffed suspect
/// waiting for a guard; plan deviation from the spec).
pub fn sense(world: &mut World, actor: EntityId, crime: Crime, b: EntityId) -> Sensed {
    if !matches!(crime, Crime::Theft | Crime::Extortion) || !world.has::<Brain>(actor) {
        return Sensed::Unseen;
    }
    let Some(robot) = powered_robot(world, b) else { return Sensed::Unseen };
    let robot_tier = world.comp::<Asset>(robot).map_or(1, |x| x.tier);
    let thief_tier = crate::systems::security::thief_tier(crate::systems::law::stealth(world, actor));
    let step = world.config.chrome.contest_step;
    if crate::systems::security::contest(thief_tier, robot_tier, step, world.rng.world()) {
        return Sensed::Unseen;
    }
    let (winner, _, died) = crate::systems::law::resolve_fight(world, robot, actor);
    if !crate::systems::law::living(world, actor) {
        return Sensed::Died;
    }
    crate::systems::law::file_report(world, crime, actor, None);
    if winner != robot || died {
        return Sensed::Unseen;
    }
    let Some(jail) = world.building_of_kind(BuildingKind::Jail) else { return Sensed::Unseen };
    if world.has::<crate::components::Sentence>(actor)
        || world.sentenced().len() >= usize::from(world.config.buildings.jail.capacity)
    {
        return Sensed::Unseen;
    }
    crate::systems::stims::confiscate(world, actor);
    let until = world.tick + crate::systems::law::sentence_ticks(world, crime);
    crate::systems::law::sentence(world, actor, crime, until, jail);
    world.resolve_reports_of(actor);
    world.stats.current.arrests += 1;
    let text = format!("{} detained {} at {}", world.name_of(robot), world.name_of(actor), world.name_of(b));
    world.push_event(crate::events::EventKind::Arrest, &[actor, b, robot], text);
    Sensed::Detained
}

/// D42: robots `corp` sold that are in service at a building it does not
/// own (posted, not wrecked): the robot's half of its Security share, as
/// `corp_brain::sold_contracts` is the contracts'.
pub fn sold_in_service(world: &World, corp: EntityId) -> usize {
    world
        .robot_sellers
        .iter()
        .filter(|&(_, &s)| s == corp)
        .filter(|&(&r, _)| {
            world.comp::<Asset>(r).is_some_and(|x| match x.loc {
                AssetLoc::Posted(b) => x.kind == AssetKind::Robot && x.condition > 0 && world.owner_of(b) != Some(corp),
                _ => false,
            })
        })
        .count()
}

/// The Security business a corp sells others: contracts and robots in service.
fn security_sold(world: &World, c: EntityId) -> usize {
    let contracts = world.comp::<Corp>(c).map_or(0, |cc| crate::systems::corp_brain::sold_contracts(world, cc, c));
    contracts + sold_in_service(world, c)
}

/// D42: the Security corp a robot for `corp` is bought from: itself when it
/// sells security, else (as `corps::cheapest_seller` deals contracts) the
/// Security corp owning an Office with the lowest price level, then the one
/// selling the fewest contracts and robots in service, then the lower id.
fn robot_seller(world: &World, corp: EntityId) -> Option<EntityId> {
    let sells = |c: EntityId| world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&Niche::Security));
    if sells(corp) {
        return Some(corp);
    }
    world
        .corps()
        .into_iter()
        .filter(|&c| sells(c) && office_of(world, c, None).is_some())
        .map(|c| {
            let level = world.comp::<Corp>(c).map_or(1.0, |cc| cc.level(Niche::Security));
            (ordered_float::OrderedFloat(level), security_sold(world, c), c)
        })
        .min()
        .map(|(_, _, c)| c)
}

/// `seller`'s Security Office nearest `near` (door Manhattan, ties the lower
/// id; any with `near` None).
fn office_of(world: &World, seller: EntityId, near: Option<crate::components::TilePos>) -> Option<EntityId> {
    world
        .buildings_of_kind(BuildingKind::SecurityOffice)
        .iter()
        .copied()
        .filter_map(|o| {
            world.comp::<Building>(o).filter(|b| b.owner == Some(seller) && !b.demolished).map(|b| (b.door, o))
        })
        .map(|(door, o)| (near.map_or(0, |n| door.manhattan(n)), o))
        .min()
        .map(|(_, o)| o)
}

/// D42, from `corp_brain::secure` per building it would contract: a robot
/// at `robot_tier_by_kind` when `price + upkeep × robot_horizon_days <
/// contract_per_guard_day × level × robot_horizon_days` and the treasury
/// holds `robot_cash_mult × price`, bought (`assets::buy`, "for Secure") at
/// the seller's Office nearest and posted at `b`; the building's contract,
/// if any, ends. A Security corp buys from its own Office (it pays only the
/// import: no markup). None when a robot is posted there already. Returns
/// true when a robot was bought.
pub fn consider_robot(world: &mut World, corp: EntityId, b: EntityId) -> bool {
    if !world.config.assets.enabled || posted_robot(world, b).is_some() {
        return false;
    }
    let Some((kind, door)) = world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.kind, bd.door)) else {
        return false;
    };
    let Some(seller) = robot_seller(world, corp) else { return false };
    let Some(office) = office_of(world, seller, Some(door)) else { return false };
    let tier = robot_tier_for(world, kind);
    let Some(list) = assets::list_price(world, AssetKind::Robot, tier) else { return false };
    let level = world.comp::<Corp>(seller).map_or(1.0, |c| c.level(Niche::Security));
    let price = if seller == corp {
        (world.config.assets.import_frac * list as f32).round() as i64
    } else {
        (list as f32 * level).round() as i64
    };
    let horizon = i64::from(world.config.robots.robot_horizon_days);
    let robot_cost = price + assets::upkeep_for(world, AssetKind::Robot, tier) * horizon;
    let contract_cost = (world.config.corps.contract_per_guard_day as f32 * level).round() as i64 * horizon;
    // The cash floor reads the list price at the seller's level even for a
    // Security corp's own robot, so `assets::buy` never finances it.
    let full = (list as f32 * level).round().max(price as f32);
    let treasury = world.purse(Some(corp));
    if robot_cost >= contract_cost || (treasury as f32) < world.config.robots.robot_cash_mult * full {
        return false;
    }
    let pick = ShopPick { kind: AssetKind::Robot, tier, used: None };
    let note = format!("for Secure ({})", world.name_of(b));
    let Ok(a) = assets::buy_noted(world, corp, office, &pick, Some(&note)) else { return false };
    assets::set_loc(world, a, AssetLoc::Posted(b));
    if seller != corp {
        world.robot_sellers.insert(a, seller);
    }
    crate::systems::corps::end_contract(world, b, "replaced by a robot");
    true
}
