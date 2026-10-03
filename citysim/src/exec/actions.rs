//! Per-action preconditions, durations, start effects (money) and
//! completion effects (everything else). Only the actions the M1 routine
//! uses are implemented; the rest fail `can_start` until their milestone.

use crate::components::{
    Brain, Building, BuildingKind, Edge, Household, Inventory, Job, MemoryKind, Needs, Position, RelKind, Skills,
    Wallet,
};
use crate::entity::EntityId;
use crate::exec::{FailReason, StepResult};
use crate::goap::ActionKind;
use crate::needs;
use crate::systems::economy;
use crate::time::Tick;
use crate::world::World;

/// Fixed durations in ticks from the Action table; work actions run to the
/// shift end and Sleep is capped by `sleep_ticks_full`.
pub fn duration(world: &World, id: EntityId, kind: ActionKind) -> Tick {
    match kind {
        ActionKind::EatFromInventory => 10,
        ActionKind::EatAtHome => 15,
        ActionKind::BuyFood => 10,
        ActionKind::Sleep => Tick::from(world.config.needs.sleep_ticks_full),
        ActionKind::Rest => 90,
        ActionKind::HaulToMarket => 40,
        ActionKind::CollectWage | ActionKind::CollectDole | ActionKind::StoreFood => 5,
        ActionKind::Drink => 45,
        ActionKind::Wander => 30,
        k if k.is_work() => {
            world.comp::<Job>(id).and_then(|j| j.shift_end(world.tick)).map_or(0, |end| end.saturating_sub(world.tick))
        }
        _ => 30,
    }
}

/// Is the agent inside a building of `kind`?
fn at(world: &World, id: EntityId, kind: BuildingKind) -> bool {
    world
        .comp::<Position>(id)
        .and_then(|p| p.building)
        .and_then(|b| world.comp::<Building>(b))
        .is_some_and(|b| b.kind == kind)
}

fn at_home(world: &World, id: EntityId) -> bool {
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    home.is_some() && world.comp::<Position>(id).is_some_and(|p| p.building == home)
}

fn home_pantry(world: &World, id: EntityId) -> u32 {
    world.comp::<Household>(id).and_then(|h| h.home).and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.stock_food)
}

/// On shift, at the employer, and this shift not yet worked.
pub fn can_work_now(world: &World, id: EntityId, job: &Job) -> bool {
    job.on_shift(world.tick_of_day())
        && job.last_shift_day != Some(job.shift_key_at(world.tick))
        && job.employer.is_some_and(|e| world.comp::<Position>(id).is_some_and(|p| p.building == Some(e)))
}

pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let inv = world.comp::<Inventory>(id);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    match kind {
        ActionKind::EatFromInventory => inv.is_some_and(|i| i.food >= 1),
        ActionKind::EatAtHome => at_home(world, id) && home_pantry(world, id) > 0,
        ActionKind::StoreFood => at_home(world, id) && inv.is_some_and(|i| i.food > i.stolen_food),
        ActionKind::BuyFood => at(world, id, BuildingKind::Market) && economy::buy_quantity(world, id) > 0,
        ActionKind::Sleep | ActionKind::Rest => {
            // Sleep on the street is allowed (homeless); Rest anywhere indoors.
            true
        }
        ActionKind::Drink => at(world, id, BuildingKind::Bar) && coins >= 2,
        ActionKind::CollectWage => {
            at(world, id, BuildingKind::Hall)
                && world
                    .comp::<Job>(id)
                    .is_some_and(|j| j.days_unpaid >= 1 && j.last_wage_attempt_day != Some(world.day()))
        }
        ActionKind::CollectDole => {
            at(world, id, BuildingKind::Hall)
                && !world.has::<Job>(id)
                && world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day != Some(world.day()))
                && world.treasury().is_some_and(|t| t.coins >= 0)
        }
        ActionKind::HaulToMarket => {
            at(world, id, BuildingKind::Farm)
                && target
                    .and_then(|t| world.comp::<Building>(t))
                    .is_some_and(|b| b.stock_food >= world.config.economy.haul_min_stock)
        }
        k if k.is_work() => {
            let Some(job) = world.comp::<Job>(id) else { return false };
            can_work_now(world, id, job) && ActionKind::work_for(job.role) == k
        }
        ActionKind::Wander => true,
        _ => false,
    }
}

/// Start effects: money moves now so an interruption cannot duplicate it.
pub fn on_start(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>) {
    match kind {
        ActionKind::BuyFood => {
            let units = economy::buy_quantity(world, id);
            let paid = economy::pay_for_food(world, id, units);
            world.pending_purchase.insert(id, (units, paid));
        }
        ActionKind::HaulToMarket => {
            // The transfer happens on pickup so an interrupted walk cannot lose the load.
            if let Some(farm) = target {
                economy::haul(world, farm);
            }
        }
        ActionKind::Drink => {
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins -= 2;
            }
            if let Some(t) = world.treasury_mut() {
                t.coins += 2; // the Bar is city-owned in v1
            }
        }
        _ => {}
    }
}

/// Effects of abandoning a step mid-way: hours already farmed still count,
/// a paid-for purchase is refunded; everything else is simply dropped.
pub fn on_abort(world: &mut World, id: EntityId, kind: ActionKind, started: Tick) {
    match kind {
        ActionKind::FarmWork => {
            if let Some(farm) = world.comp::<Job>(id).and_then(|j| j.employer) {
                let now = world.tick;
                economy::accrue_farm_work(world, id, farm, now.saturating_sub(started));
            }
        }
        ActionKind::BuyFood => {
            if let Some((_, paid)) = world.pending_purchase.remove(&id) {
                if let Some(w) = world.comp_mut::<Wallet>(id) {
                    w.coins += paid;
                }
                if let Some(t) = world.treasury_mut() {
                    t.coins -= paid;
                }
            }
        }
        _ => {}
    }
}

/// Some actions end before their nominal duration.
pub fn finishes_early(world: &World, id: EntityId, kind: ActionKind) -> bool {
    match kind {
        ActionKind::Sleep => {
            world.comp::<Needs>(id).is_some_and(|n| n.energy >= 0.9)
                || crate::exec::routine::must_leave_for_work(world, id)
        }
        ActionKind::Rest => crate::exec::routine::must_leave_for_work(world, id),
        // Waiting at the workplace for the shift: start the moment it begins.
        ActionKind::Wander => world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j)),
        _ => false,
    }
}

/// Completion effects. Returns `Failed(StockGone)` when the resource the
/// action needed has vanished since it started.
pub fn on_complete(
    world: &mut World,
    id: EntityId,
    kind: ActionKind,
    _target: Option<EntityId>,
    started: Tick,
    now: Tick,
) -> StepResult {
    let cfg = world.config.needs.clone();
    match kind {
        ActionKind::EatFromInventory => {
            let Some(inv) = world.comp_mut::<Inventory>(id) else { return StepResult::Failed(FailReason::StockGone) };
            if inv.food == 0 {
                return StepResult::Failed(FailReason::StockGone);
            }
            inv.food -= 1;
            inv.stolen_food = inv.stolen_food.min(inv.food);
            if let Some(n) = world.comp_mut::<Needs>(id) {
                needs::eat(n, &cfg);
            }
            world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            StepResult::Done
        }
        ActionKind::EatAtHome => {
            let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let Some(b) = world.comp_mut::<Building>(home) else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if b.stock_food == 0 {
                return StepResult::Failed(FailReason::StockGone);
            }
            b.stock_food -= 1;
            if let Some(n) = world.comp_mut::<Needs>(id) {
                needs::eat(n, &cfg);
            }
            world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            StepResult::Done
        }
        ActionKind::StoreFood => {
            let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let cap = world.config.buildings.home.stock_cap;
            let room = world.comp::<Building>(home).map_or(0, |b| cap.saturating_sub(b.stock_food));
            let Some(inv) = world.comp_mut::<Inventory>(id) else { return StepResult::Failed(FailReason::StockGone) };
            let movable = (inv.food - inv.stolen_food).min(room);
            inv.food -= movable;
            if let Some(b) = world.comp_mut::<Building>(home) {
                b.stock_food += movable;
            }
            StepResult::Done
        }
        ActionKind::BuyFood => {
            let (units, paid) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            if economy::take_food(world, id, units, paid) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        ActionKind::Sleep => {
            if at_home(world, id) {
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.safety = (n.safety + 0.3).min(1.0);
                }
                if spouse_in_same_home(world, id) {
                    if let Some(n) = world.comp_mut::<Needs>(id) {
                        n.intimacy = (n.intimacy + 0.4).min(1.0);
                    }
                }
            } else if let Some(n) = world.comp_mut::<Needs>(id) {
                n.safety = (n.safety - 0.1).max(0.0);
            }
            StepResult::Done
        }
        ActionKind::Rest => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.energy = (n.energy + 0.1).min(1.0);
            }
            StepResult::Done
        }
        ActionKind::Drink => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.belonging = (n.belonging + 0.15).min(1.0);
            }
            world.remember(id, MemoryKind::Socialised, None, 0.2, 0.3, false);
            StepResult::Done
        }
        ActionKind::CollectWage => {
            economy::collect_wage(world, id);
            StepResult::Done
        }
        ActionKind::CollectDole => {
            economy::collect_dole(world, id);
            StepResult::Done
        }
        ActionKind::HaulToMarket => StepResult::Done, // moved on pickup, see on_start
        ActionKind::FarmWork => {
            let farm = world.comp::<Job>(id).and_then(|j| j.employer);
            if let Some(farm) = farm {
                economy::accrue_farm_work(world, id, farm, now.saturating_sub(started));
            }
            if let Some(s) = world.comp_mut::<Skills>(id) {
                s.farming = (s.farming + 0.002).min(1.0);
            }
            end_shift(world, id);
            // Hauling: appended to the plan if the farm has enough stock.
            let min = world.config.economy.haul_min_stock;
            let stock = farm.and_then(|f| world.comp::<Building>(f)).map_or(0, |b| b.stock_food);
            if stock >= min {
                if let (Some(farm), Some(b)) = (farm, world.comp_mut::<Brain>(id)) {
                    let at = usize::from(b.plan_step) + 1;
                    if let Some(plan) = b.plan.as_mut() {
                        // Insert right after this step: the farmer is still at the farm.
                        let at = at.min(plan.steps.len());
                        plan.steps.insert(
                            at,
                            crate::components::ActionInstance {
                                action: ActionKind::GoTo(crate::goap::LocationKey::Market),
                                target: None,
                                tile: None,
                            },
                        );
                        plan.steps.insert(
                            at,
                            crate::components::ActionInstance {
                                action: ActionKind::HaulToMarket,
                                target: Some(farm),
                                tile: None,
                            },
                        );
                    }
                }
            }
            StepResult::Done
        }
        ActionKind::GuardJail | ActionKind::ClerkWork | ActionKind::BartendWork | ActionKind::TendGraves => {
            end_shift(world, id);
            StepResult::Done
        }
        ActionKind::Wander => StepResult::Done,
        _ => StepResult::Failed(FailReason::PreconditionLost),
    }
}

/// Shift end: the shift is marked worked (so Work stays pending while it
/// runs and a resumed shift is not counted twice) and a day of wages is owed.
fn end_shift(world: &mut World, id: EntityId) {
    let tick = world.tick;
    if let Some(j) = world.comp_mut::<Job>(id) {
        j.last_shift_day = Some(j.shift_key_at(tick.saturating_sub(1)));
        j.days_unpaid = j.days_unpaid.saturating_add(1);
    }
    economy::maybe_quit(world, id);
}

fn spouse_in_same_home(world: &World, id: EntityId) -> bool {
    let Some(home) = world.comp::<Position>(id).and_then(|p| p.building) else { return false };
    world.edges.iter().any(|(&(a, b), e): (&(EntityId, EntityId), &Edge)| {
        e.kind == RelKind::Spouse
            && (a == id || b == id)
            && world.comp::<Position>(if a == id { b } else { a }).is_some_and(|p| p.building == Some(home))
    })
}
