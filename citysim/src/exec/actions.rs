//! Per-action preconditions, durations, start effects (money) and
//! completion effects (everything else). Only the actions the M1 routine
//! uses are implemented; the rest fail `can_start` until their milestone.

use crate::components::{
    Brain, Building, BuildingKind, Household, Inventory, Job, MemoryKind, Needs, Position, Skills, TilePos, Wallet,
};
use crate::entity::EntityId;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, StealSource};
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
        ActionKind::StealFood(StealSource::Market) => 20,
        ActionKind::StealFood(StealSource::Home) => 20,
        ActionKind::StealFood(StealSource::Warehouse) => 25,
        ActionKind::Forage => 45,
        ActionKind::Beg => 30,
        ActionKind::SellFood => 10,
        ActionKind::ReportCrime => 10,
        ActionKind::PatrolLeg => 20,
        ActionKind::Arrest => 10,
        ActionKind::HideFromLaw => 120,
        ActionKind::Chat | ActionKind::Flirt | ActionKind::JoinGang => 30,
        ActionKind::Propose | ActionKind::SplitLoot => 10,
        ActionKind::Extort => 20,
        ActionKind::Fence | ActionKind::Attack => 15,
        ActionKind::CarryCorpse => 20,
        ActionKind::BuryCorpse => 60,
        // M12 D31: until the expedition leaves (a gang's raid_at, a riot's muster_at).
        ActionKind::Muster => crate::systems::raid::departure(world, id).map_or(0, |t| t.saturating_sub(world.tick)),
        ActionKind::Brawl => 15,
        ActionKind::Register => 30,
        ActionKind::CheckIn => 5,
        ActionKind::Occupy => 10,
        // M13 D29 (+120 for an implant: it is installed in the same action), D26.
        ActionKind::BuyAsset => {
            let implant = world.comp::<Brain>(id).and_then(|b| b.shop_pick.as_ref()).is_some_and(|p| {
                p.kind.is_implant()
                    || p.used
                        .and_then(|u| world.comp::<crate::components::Asset>(u))
                        .is_some_and(|x| x.kind.is_implant())
            });
            if implant {
                140
            } else {
                20
            }
        }
        ActionKind::StealVehicle => 15,
        // M13 D34, D35, D36.
        ActionKind::Install => 120,
        ActionKind::Therapy | ActionKind::Uninstall => 60,
        ActionKind::Strip => 10,
        ActionKind::Rip => {
            if crate::systems::chrome::dragging(world, id).is_some() {
                60
            } else {
                40
            }
        }
        ActionKind::Abduct => 5,
        // M13 D38/D39.
        ActionKind::PickUp | ActionKind::BuyStims => 5,
        ActionKind::Deal | ActionKind::Detox => 240,
        ActionKind::UseStim => 2,
        // M14 Actions table.
        ActionKind::JackIn => 2,
        ActionKind::SellData => 10,
        ActionKind::UpgradeDeck => 30,
        // L1: a haul of scrap; office hours to the shift's end.
        ActionKind::Scavenge => 60,
        ActionKind::Meeting => crate::systems::life::exec_shift_left(world),
        // L2 L14 (plan 2.1): the leisure steps.
        ActionKind::Enjoy => 90,
        ActionKind::Gamble | ActionKind::EatOut | ActionKind::Preach => 30,
        ActionKind::HangOut => world.config.leisure.hangout_ticks,
        ActionKind::Collect => 10,
        // M16a (plan C10, C15).
        ActionKind::Network => 30,
        // Real economy E27, E28: a meal at the kitchen, a gift at the door.
        ActionKind::EatAlms => 30,
        ActionKind::Donate => 5,
        ActionKind::Guard => Tick::from(world.config.fixers.guard_hours) * crate::time::TICKS_PER_HOUR,
        // M15 W21/W22/W35.
        ActionKind::AskAround => 20,
        ActionKind::StakeOut => {
            let guard =
                world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::GuardBody);
            if guard {
                Tick::from(world.config.grudges.guard_hours) * crate::time::TICKS_PER_HOUR
            } else {
                Tick::from(world.config.hunt.stakeout_hours) * crate::time::TICKS_PER_HOUR
            }
        }
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

/// Inside a Clinic with a Ripperdoc on shift (M13 D34).
fn in_open_clinic(world: &World, id: EntityId) -> bool {
    world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| {
        world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Clinic)
            && crate::systems::assets::seller_open(world, b)
    })
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
        && if job.role == crate::components::Role::Sanitation && crate::systems::jobs::sweep_on(world) {
            // L2 L8: a sweeper works out on its beat's streets.
            crate::systems::jobs::on_beat(world, id)
        } else {
            job.employer.is_some_and(|e| world.comp::<Position>(id).is_some_and(|p| p.building == Some(e)))
        }
}

pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let inv = world.comp::<Inventory>(id);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    // M12 D33: no trade in a building a riot closed.
    let closed_here = || world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| world.is_closed(b));
    match kind {
        ActionKind::EatFromInventory => inv.is_some_and(|i| i.food >= 1),
        ActionKind::EatAtHome => at_home(world, id) && home_pantry(world, id) > 0,
        ActionKind::StoreFood => at_home(world, id) && inv.is_some_and(|i| i.food > i.stolen_food),
        ActionKind::BuyFood => {
            at(world, id, BuildingKind::Market)
                && !closed_here()
                && economy::buy_quantity(world, id, world.local(id, BuildingKind::Market)) > 0
        }
        ActionKind::Sleep | ActionKind::Rest => {
            // Sleep on the street is allowed (homeless); Rest anywhere indoors.
            true
        }
        // L2 L14: a Club's or Den's bar too, with leisure on (open, door kept).
        ActionKind::Drink => {
            (at(world, id, BuildingKind::Bar) && coins >= 2 && !closed_here())
                || (crate::systems::leisure::drink_venue(world, id)
                    && coins >= 2
                    && world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| {
                        crate::systems::leisure::open(world, b) && crate::systems::leisure::door_ok(world, id, b)
                    }))
        }
        // L2 L14: the scripted leisure steps.
        k if k.is_leisure_step() => crate::systems::leisure::can_start(world, id, k, target),
        // M16a (plan C10): the scripted contract steps.
        k if k.is_contract_step() => crate::systems::contracts::can_start(world, id, k, target),
        // Real economy E27, E28: the scripted charity steps.
        k if k.is_charity_step() => crate::systems::charity::can_start(world, id, k, target),
        // M11 D13: at the wage desk (the Hall for a city job, else the workplace).
        ActionKind::CollectWage => {
            world.comp::<Position>(id).is_some_and(|p| p.building.is_some() && p.building == world.wage_desk(id))
                && world.comp::<Job>(id).is_some_and(|j| j.wage_collectable(world.day()))
        }
        ActionKind::CollectDole => {
            at(world, id, BuildingKind::Hall)
                // `economy::dole_eligible` (has no Job), the planner's reading.
                && crate::systems::economy::dole_eligible(world, id)
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
            can_work_now(world, id, job) && crate::exec::routine::work_action(world, job.role) == k
        }
        ActionKind::Wander => true,
        // L1: out on the street; office hours inside the HQ.
        ActionKind::Scavenge => world.comp::<Position>(id).is_some_and(|p| p.building.is_none()),
        ActionKind::Meeting => target.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == target,
        ActionKind::Register => at(world, id, BuildingKind::Hall) && crate::systems::founding::can_found(world, id),
        // M12 D21: inside a standing Hotel (bed and coins re-checked at completion).
        ActionKind::CheckIn => world
            .comp::<Position>(id)
            .and_then(|p| p.building)
            .is_some_and(|b| crate::systems::street::is_hotel(world, b)),
        // M12 D27: inside the bound derelict.
        ActionKind::Occupy => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            here.is_some_and(|b| crate::systems::street::is_derelict(world, b)) && (target.is_none() || target == here)
        }
        // M13 D29: inside an open seller with a pick to buy.
        ActionKind::BuyAsset => {
            world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| {
                Some(b) == target && crate::systems::assets::seller_open(world, b) && !world.is_closed(b)
            }) && world.comp::<Brain>(id).is_some_and(|b| b.shop_pick.is_some())
        }
        // M13 D34: inside an open Clinic (the Install with the gang's
        // implant reserved; Uninstall with chrome in).
        ActionKind::Install => {
            in_open_clinic(world, id)
                && target.is_none_or(|t| world.comp::<Position>(id).and_then(|p| p.building) == Some(t))
                && crate::systems::chrome::reserved_implant(world, id).is_some()
        }
        ActionKind::Therapy => in_open_clinic(world, id),
        ActionKind::Uninstall => {
            in_open_clinic(world, id) && world.comp::<crate::components::Kit>(id).is_some_and(|k| k.chrome)
        }
        // M13 D38: inside the gang's Hideout with doses in its stock.
        ActionKind::PickUp => {
            let hideout = world.gang_of(id).and_then(|g| world.hideout_of(g));
            hideout.is_some()
                && world.comp::<Position>(id).and_then(|p| p.building) == hideout
                && hideout.is_some_and(|h| world.stock(h, crate::components::Good::Stims) > 0)
        }
        // M13 D38: inside the bound deal Bar with doses to sell.
        ActionKind::Deal => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            here.is_some() && here == target && !closed_here() && inv.is_some_and(|i| i.stims > 0)
        }
        // M13 D38/D40: inside the bound source while it sells, with the coins for a dose.
        ActionKind::BuyStims => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            here.is_some()
                && here == target
                && here.and_then(|b| crate::systems::stims::source_price(world, b)).is_some_and(|p| coins >= p)
        }
        ActionKind::UseStim => inv.is_some_and(|i| i.stims > 0),
        // M13 D39: inside an open Clinic.
        ActionKind::Detox => in_open_clinic(world, id),
        // M14 V11: inside the order's chair, the order due, a deck in hand.
        ActionKind::JackIn => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            world.config.virt.enabled
                && world.run_orders.get(&id).is_some_and(|o| Some(o.chair) == here && world.tick >= o.not_before)
                && world.comp::<crate::components::Kit>(id).is_some_and(|k| k.deck.is_some())
                && !world.runner_of.contains_key(&id)
        }
        // M14 V29: inside a Data buyer's Lab with Data on the deck.
        ActionKind::SellData => {
            world
                .comp::<Position>(id)
                .and_then(|p| p.building)
                .is_some_and(|b| crate::systems::tech::is_data_buyer_lab(world, b) && !world.is_closed(b))
                && crate::systems::virt::deck_data(world, id) > 0
        }
        // M14 V36: inside an open deck seller with an upgrade picked.
        ActionKind::UpgradeDeck => {
            world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| {
                Some(b) == target && crate::systems::assets::seller_open(world, b) && !world.is_closed(b)
            }) && world.comp::<Brain>(id).and_then(|b| b.shop_pick.as_ref()).is_some_and(|p| p.upgrade)
        }
        // M13 D35: beside a body this agent may strip.
        ActionKind::Strip => target.is_some_and(|c| {
            crate::systems::chrome::may_loot(world, id, c) && crate::systems::law::near(world, id, c, 1)
        }),
        // M13 D35/D36: beside a body with chrome, or home with the one in tow.
        ActionKind::Rip => target.is_some_and(|b| {
            let home = world.gang_of(id).and_then(|g| world.hideout_of(g));
            let at_home = home.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == home;
            let towed = crate::systems::chrome::dragging(world, id) == Some(b)
                || world.comp::<Brain>(id).and_then(|x| x.carrying_corpse) == Some(b);
            (towed && at_home)
                || (crate::systems::chrome::may_rip(world, id)
                    && world.comp::<crate::components::Corpse>(b).is_some_and(|c| !c.buried)
                    && crate::systems::law::near(world, id, b, 1)
                    && !crate::systems::chrome::installed(world, b).is_empty())
        }),
        // M13 D36: beside the Harvest target, not already in someone's hands.
        ActionKind::Abduct => target.is_some_and(|v| {
            crate::systems::law::living(world, v)
                && crate::systems::law::near(world, id, v, 1)
                && world.comp::<Brain>(v).is_some_and(|b| b.cuffed_by.is_none())
        }),
        // M13 D26: standing at the bound vehicle's door, and it is still there.
        ActionKind::StealVehicle => target.is_some_and(|v| {
            crate::systems::vehicles::vehicle_stand(world, v)
                .is_some_and(|t| world.comp::<Position>(id).is_some_and(|p| p.building.is_none() && p.tile == t))
        }),
        ActionKind::CarryCorpse => target.is_some_and(|c| {
            world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried)
                && crate::systems::law::near(world, id, c, 1)
                && world.comp::<Brain>(id).is_some_and(|b| b.carrying_corpse.is_none())
        }),
        ActionKind::BuryCorpse => {
            at(world, id, BuildingKind::Cemetery)
                && target.is_some_and(|c| world.comp::<Brain>(id).is_some_and(|b| b.carrying_corpse == Some(c)))
        }
        // Planned toward a partner at a venue; starting needs them in the room.
        ActionKind::Flirt | ActionKind::Propose => target.is_some_and(|t| {
            world.comp::<Position>(t).and_then(|p| p.building).is_some()
                && world.comp::<Position>(t).and_then(|p| p.building)
                    == world.comp::<Position>(id).and_then(|p| p.building)
        }),
        // Symbolic preconditions (goap::ActionKind::preconditions) cover these.
        ActionKind::StealFood(_)
        | ActionKind::Forage
        | ActionKind::Beg
        | ActionKind::SellFood
        | ActionKind::ReportCrime
        | ActionKind::PatrolLeg
        | ActionKind::Arrest
        | ActionKind::HideFromLaw
        | ActionKind::Chat
        | ActionKind::JoinGang
        | ActionKind::Extort
        | ActionKind::SplitLoot
        | ActionKind::Fence
        | ActionKind::Attack
        | ActionKind::Muster
        | ActionKind::Brawl => true,
        _ => false,
    }
}

/// Start effects: money moves now so an interruption cannot duplicate it.
pub fn on_start(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>) {
    match kind {
        // Sleeping at home beside a spouse: intimacy +0.4, at the start so an
        // interrupted night still counts.
        ActionKind::Sleep => {
            // Once a night: a sleep interrupted and resumed does not stack it.
            let night = world.tick.saturating_add(crate::time::TICKS_PER_DAY / 2) / crate::time::TICKS_PER_DAY;
            let fresh = world.comp::<Brain>(id).is_some_and(|b| b.last_spouse_night != Some(night));
            if at_home(world, id) {
                world.mark_day(id, crate::components::trace_flags::SLEPT_AT_HOME);
            }
            if fresh && at_home(world, id) && spouse_in_same_home(world, id) {
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.intimacy = (n.intimacy + 0.4).min(1.0);
                }
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.last_spouse_night = Some(night);
                }
            }
        }
        ActionKind::BuyFood => {
            let market = world.local(id, BuildingKind::Market);
            let units = economy::buy_quantity(world, id, market);
            let paid = economy::pay_for_food(world, id, market, units);
            // L2 shadow fixes item 4: the units move with the coins (an
            // interrupted purchase was refunded and bought again: 12 food
            // flows for 6 meals); `(0, 0)` marks a shelf found empty.
            if crate::systems::fixes::item(world, 4) {
                let taken = economy::take_food(world, id, market, units, paid);
                world.pending_purchase.insert(id, if taken { (units, 0) } else { (0, 0) });
                return;
            }
            world.pending_purchase.insert(id, (units, paid));
        }
        ActionKind::Wander => {
            // Effect `at = Street`: step out of whatever building we are in.
            world.leave_building(id);
        }
        ActionKind::HaulToMarket => {
            // The transfer happens on pickup so an interrupted walk cannot lose the load.
            if let Some(farm) = target {
                // M13 D23: a fleet truck (else car) parked here carries
                // `haul[kind]` batches, and the plan brings it home.
                let mult = crate::systems::vehicles::claim_haul(world, id, farm);
                economy::haul(world, farm, mult);
                if mult.is_some() {
                    if let Some(b) = world.comp_mut::<Brain>(id) {
                        let at = usize::from(b.plan_step) + 2;
                        if let Some(plan) = b.plan.as_mut() {
                            let at = at.min(plan.steps.len());
                            plan.steps.insert(
                                at,
                                crate::components::ActionInstance {
                                    action: ActionKind::GoTo(crate::goap::LocationKey::Farm),
                                    target: Some(farm),
                                    tile: None,
                                },
                            );
                        }
                    }
                }
            }
        }
        // M14 V5: a public terminal's fee to the Bar's or Hotel's owner
        // (`Flow::Terminal`, taxed), at the start (money moves at start).
        ActionKind::JackIn => crate::systems::virt::pay_terminal(world, id),
        // M15 W22/W35: the stake-out's timeout; a guard stands over the body.
        ActionKind::StakeOut => {
            let guard =
                world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::GuardBody);
            match (guard, target) {
                (true, Some(c)) => crate::systems::grudges::start_guard(world, id, c),
                _ => {
                    let until = world.tick + duration(world, id, kind);
                    crate::systems::hunt::start_stakeout(world, id, until);
                }
            }
        }
        // M13 D38: the dealer is registered at the Bar for the shift.
        ActionKind::Deal => {
            if let Some(bar) = target {
                crate::systems::stims::start_deal(world, id, bar);
            }
        }
        k if k.is_leisure_step() => crate::systems::leisure::on_start(world, id, k, target),
        // M16a (plan C32): the guard is on post.
        ActionKind::Guard => crate::systems::contracts::on_guard_start(world, id, target),
        ActionKind::Drink => {
            // M11: the Bar's owner takes the 2 coins (the Treasury owns a city Bar).
            let bar = world.comp::<Position>(id).and_then(|p| p.building);
            let owner = bar.and_then(|b| world.owner_of(b));
            let paid =
                crate::systems::ownership::pay(world, Some(id), owner, 2, crate::systems::ownership::Flow::Drink);
            if let Some(b) = bar {
                crate::systems::ownership::credit(world, b, paid);
            }
        }
        _ => {}
    }
}

/// Effects of abandoning a step mid-way: hours already farmed still count,
/// a paid-for purchase is refunded; everything else is simply dropped.
pub fn on_abort(world: &mut World, id: EntityId, kind: ActionKind, started: Tick) {
    // L2 shadow fixes item 2: a shift cut past half is paid pro rata.
    crate::systems::fixes::work_aborted(world, id, kind, started);
    // Item 4: a started purchase is atomic: nothing is refunded; a meal out
    // is eaten, an entry's fun is had for the time spent.
    if crate::systems::fixes::item(world, 4) && crate::systems::fixes::paid_step(kind) {
        match kind {
            ActionKind::BuyFood => {
                world.pending_purchase.remove(&id);
            }
            _ => crate::systems::leisure::abort_paid(world, id, kind, started),
        }
        return;
    }
    match kind {
        ActionKind::FarmWork => {
            if let Some(farm) = world.comp::<Job>(id).and_then(|j| j.employer) {
                let now = world.tick;
                economy::accrue_farm_work(world, id, farm, now.saturating_sub(started));
            }
        }
        // L2 L8: the hours worked count, as FarmWork's.
        ActionKind::FabWork => {
            if let Some(fab) = world.comp::<Job>(id).and_then(|j| j.employer) {
                let now = world.tick;
                crate::systems::jobs::accrue_fab_work(world, id, fab, now.saturating_sub(started));
            }
        }
        ActionKind::Sweep => {
            let now = world.tick;
            crate::systems::jobs::sweep_done(world, id, now.saturating_sub(started));
        }
        ActionKind::BuyFood => {
            if let Some((_, paid)) = world.pending_purchase.remove(&id) {
                let market = world.local(id, BuildingKind::Market);
                economy::refund_food(world, id, market, paid);
            }
        }
        // M13 D38: a dealer who leaves the Bar is no longer dealing there.
        ActionKind::Deal => crate::systems::stims::end_deal(world, id),
        // L2 L14: a paid entry or meal refunded; a HangOut leaves its spot.
        k if k.is_leisure_step() => crate::systems::leisure::on_abort(world, id, k),
        // M16a (plan C32): off post.
        ActionKind::Guard => {
            let c = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target);
            crate::systems::contracts::on_guard_end(world, id, c);
        }
        // M15 W35: a guard who walks away stands over the body no more.
        ActionKind::StakeOut => {
            let c = world.comp::<Brain>(id).and_then(|b| b.plan.as_ref()).and_then(|p| p.target);
            if let Some(c) = c {
                crate::systems::grudges::end_guard(world, id, c);
            }
        }
        _ => {}
    }
}

/// Some actions end before their nominal duration.
pub fn finishes_early(world: &World, id: EntityId, kind: ActionKind, started: Tick) -> bool {
    match kind {
        // L1: a committed sleep: the commute gate cannot cut the first
        // `sleep_min_ticks` (a long commute made it true most of the day,
        // and the worker slept in one-minute fragments at energy 0).
        ActionKind::Sleep => {
            // L2 shadow fixes item 3: a sleep runs to `sleep_wake_energy`.
            let wake =
                if crate::systems::fixes::sleep_commit(world) { world.config.life.sleep_wake_energy } else { 0.9 };
            world.comp::<Needs>(id).is_some_and(|n| n.energy >= wake)
                || (crate::exec::routine::must_leave_for_work(world, id)
                    && (!world.config.life.enabled
                        || world.tick.saturating_sub(started) >= world.config.life.sleep_min_ticks))
        }
        ActionKind::Meeting => crate::systems::life::exec_shift_over(world),
        ActionKind::Rest => {
            crate::exec::routine::must_leave_for_work(world, id)
                || world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j))
        }
        // Waiting at the workplace for the shift: start the moment it begins.
        ActionKind::Wander => world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j)),
        // The order changed: the muster breaks up (on_complete then fails the step).
        ActionKind::Muster => crate::systems::raid::raid_done(world, id),
        // M15 W22: the target came into reach; W35: the body is gone or buried.
        ActionKind::StakeOut => {
            let guard = world
                .comp::<Brain>(id)
                .and_then(|b| b.plan.as_ref())
                .filter(|p| p.goal == crate::components::GoalKind::GuardBody);
            match guard {
                Some(p) => p.target.is_none_or(|c| {
                    world.comp::<crate::components::Corpse>(c).is_none_or(|k| k.buried || k.stripped)
                        || !crate::systems::law::near(world, id, c, 1)
                }),
                None => crate::systems::hunt::contact(world, id),
            }
        }
        _ => false,
    }
}

/// Completion effects. Returns `Failed(StockGone)` when the resource the
/// action needed has vanished since it started.
pub fn on_complete(
    world: &mut World,
    id: EntityId,
    kind: ActionKind,
    target: Option<EntityId>,
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
            world.mark_day(id, crate::components::trace_flags::ATE);
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
            world.release_all(id);
            if let Some(n) = world.comp_mut::<Needs>(id) {
                needs::eat(n, &cfg);
            }
            world.mark_day(id, crate::components::trace_flags::ATE);
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
            world.release_all(id);
            // L2 shadow fixes item 4: taken at the start already.
            if crate::systems::fixes::item(world, 4) {
                return if units > 0 { StepResult::Done } else { StepResult::Failed(FailReason::StockGone) };
            }
            let market = world.local(id, BuildingKind::Market);
            if economy::take_food(world, id, market, units, paid) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        ActionKind::Sleep => {
            // A gang member holing up at their own Hideout sleeps at home.
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let hideout_home = here.is_some()
                && (crate::systems::gang::holes_up_at(world, id) == here
                    // L1: a member bedding down at the Hideout as its nearer bed.
                    || (world.config.life.enabled && world.gang_of(id).and_then(|g| world.hideout_of(g)) == here));
            // M12 D21: a booked Hotel bed is a bed (and marks HOTEL).
            let hotel = here.is_some() && crate::systems::street::booked_hotel(world, id) == here;
            let squat = here.is_some() && world.comp::<crate::components::Squatter>(id).map(|s| s.building) == here;
            // M12 D18: litter at the door (rough: where the agent lies) costs sleep's safety.
            let penalty = world.config.litter.sleep_penalty;
            let litter_at = |w: &World, t: TilePos| -> f32 {
                if crate::systems::litter::enabled(w) {
                    f32::from(crate::systems::litter::at(w, t)) / 255.0
                } else {
                    0.0
                }
            };
            if at_home(world, id) || hideout_home || hotel {
                world.mark_day(id, crate::components::trace_flags::SLEPT_AT_HOME);
                if hotel {
                    world.mark_day(id, crate::components::trace_flags::HOTEL);
                }
                // M10: +0.2 / +0.3 / +0.4 by the building's tier (Sump, Mid, Spire).
                // Fix pass: the street outside the door (the door tile is inside
                // the building's rect, where no litter ever lands).
                let (tier, door) = here
                    .and_then(|b| world.comp::<Building>(b))
                    .map_or((1, None), |b| (b.tier, Some(world.outside_door(b))));
                let dirt = door.map_or(0.0, |d| litter_at(world, d));
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.safety = (n.safety + 0.2 + 0.1 * f32::from(tier) - penalty * dirt).clamp(0.0, 1.0);
                }
                // (the spouse's intimacy bonus is granted at Sleep start: most
                // nights' sleep is cut short by the morning shift, not completed)
            } else if squat {
                // M12 D21: a squat is shelter, no tier.
                world.mark_day(id, crate::components::trace_flags::SQUAT);
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.safety = (n.safety + 0.1).min(1.0);
                }
            } else {
                let dirt = world.comp::<Position>(id).map_or(0.0, |p| litter_at(world, p.tile));
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.safety = (n.safety - 0.1 - penalty * dirt).max(0.0);
                }
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
            // M15 W7: the rumour mill, one exchange each way with a co-drinker.
            crate::systems::gossip::drink(world, id);
            // L2 L14: a drink's fun; a Club's or Den's visit.
            crate::systems::leisure::drink_fun(world, id);
            crate::systems::leisure::note_drink(world, id);
            StepResult::Done
        }
        // L2 L14: the leisure steps' completions.
        k if k.is_leisure_step() => {
            crate::systems::leisure::on_complete(world, id, k, target, started);
            StepResult::Done
        }
        // M16a (plan C10, C15): a regular's stamp; a Guard's day kept.
        ActionKind::Network => crate::systems::contracts::on_network(world, id, target),
        // Real economy E27, E28: the meal served, the gift given.
        ActionKind::EatAlms => crate::systems::charity::on_eat_alms(world, id, target),
        ActionKind::Donate => crate::systems::charity::on_donate(world, id, target),
        ActionKind::Guard => crate::systems::contracts::on_guard_done(world, id, target),
        ActionKind::CollectWage => {
            economy::collect_wage(world, id);
            StepResult::Done
        }
        ActionKind::CollectDole => {
            economy::collect_dole(world, id);
            StepResult::Done
        }
        // L1: the Recycler buys the haul (the Treasury pays; a broke city pays nothing).
        ActionKind::Scavenge => scavenge(world, id),
        // L1: office hours kept today.
        ActionKind::Meeting => {
            let today = world.day();
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.office_day = Some(today);
            }
            StepResult::Done
        }
        // M11 D25: no Lot or coins left by now fails the step.
        ActionKind::Register => match crate::systems::founding::register(world, id) {
            Ok(_) => StepResult::Done,
            Err(_) => StepResult::Failed(FailReason::PreconditionLost),
        },
        // M12 D21: a bed and the coins re-checked; else the plan fails.
        ActionKind::CheckIn => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            match here.map(|h| crate::systems::street::check_in(world, id, h)) {
                Some(Ok(())) => StepResult::Done,
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        // M12 D27/D38: a squat (or a blow of the gang's claim on it).
        ActionKind::Occupy => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let Some(b) = here else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if crate::systems::gang::serving_squat(world, id) {
                crate::systems::gang::squat_claim(world, id, b);
                return StepResult::Done;
            }
            match crate::systems::street::occupy(world, id, b) {
                Ok(()) => StepResult::Done,
                Err(_) => StepResult::Failed(FailReason::PreconditionLost),
            }
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
            let collect_here = farm.is_some() && world.wage_desk(id) == farm;
            // Hauling: appended to the plan if the farm has enough stock.
            let min = world.config.economy.haul_min_stock;
            let stock = farm.and_then(|f| world.comp::<Building>(f)).map_or(0, |b| b.stock_food);
            if stock >= min {
                if let (Some(farm), Some(b)) = (farm, world.comp_mut::<Brain>(id)) {
                    let mut at = usize::from(b.plan_step) + 1;
                    if let Some(plan) = b.plan.as_mut() {
                        // Insert right after this step: the farmer is still at the
                        // farm. M11 D13: a corp farmer collects at the Farm, so
                        // the haul goes after that wage step.
                        if plan.steps.get(at).is_some_and(|s| s.action == ActionKind::CollectWage) && collect_here {
                            at += 1;
                        }
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
        // L2 L8: Parts into the Fab, then the shift's end.
        ActionKind::FabWork => {
            if let Some(fab) = world.comp::<Job>(id).and_then(|j| j.employer) {
                crate::systems::jobs::accrue_fab_work(world, id, fab, now.saturating_sub(started));
            }
            if let Some(s) = world.comp_mut::<Skills>(id) {
                s.farming = (s.farming + 0.002).min(1.0);
            }
            end_shift(world, id);
            StepResult::Done
        }
        // L2 L8: the beat's dirtiest streets cleaned, then the shift's end.
        ActionKind::Sweep => {
            crate::systems::jobs::sweep_done(world, id, now.saturating_sub(started));
            end_shift(world, id);
            StepResult::Done
        }
        ActionKind::Wander => StepResult::Done,
        ActionKind::ReportCrime => report_crime(world, id),
        ActionKind::PatrolLeg => {
            let legs_per_shift = world.config.crime.patrol_legs_per_shift;
            let legs = world.comp_mut::<Brain>(id).map(|b| {
                b.patrol_legs = b.patrol_legs.saturating_add(1);
                b.patrol_legs
            });
            // Five legs complete the shift; so does the clock running out mid-loop.
            let shift_over = world.comp::<Job>(id).is_some_and(|j| !j.on_shift(world.tick_of_day()));
            if legs.is_some_and(|l| l >= legs_per_shift) || shift_over {
                end_shift(world, id);
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.patrol_legs = 0;
                    b.patrol_route.clear();
                }
            }
            StepResult::Done
        }
        ActionKind::Arrest => {
            let Some(suspect) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if crate::systems::law::arrest(world, id, suspect) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::PreconditionLost)
            }
        }
        ActionKind::HideFromLaw => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.safety = (n.safety + 0.4).min(1.0);
            }
            StepResult::Done
        }
        ActionKind::Chat => {
            let Some(partner) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            if !crate::systems::law::near(world, id, partner, 1)
                && world.comp::<Position>(id).and_then(|p| p.building)
                    != world.comp::<Position>(partner).and_then(|p| p.building)
            {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            let gain = world.config.needs.chat_belonging;
            let halved = world.comp::<crate::components::Mood>(id).is_some_and(|m| m.value < -0.3);
            for who in [id, partner] {
                if let Some(n) = world.comp_mut::<Needs>(who) {
                    n.belonging = (n.belonging + gain).min(1.0);
                }
                world.remember(
                    who,
                    MemoryKind::Socialised,
                    Some(if who == id { partner } else { id }),
                    0.2,
                    0.2,
                    false,
                );
            }
            let m = crate::systems::social::similarity_mult(world, id, partner);
            let step = world.config.social.affinity_per_hour * m * if halved { 0.5 } else { 1.0 };
            crate::systems::social::adjust(world, id, partner, step, 0.02);
            // M15 W7: each legacy gossip is followed by the new exchange
            // (heard only; no edge, no world draw).
            crate::systems::social::gossip(world, id, partner);
            crate::systems::gossip::exchange(world, id, partner, crate::systems::gossip::Venue::Chat);
            crate::systems::social::gossip(world, partner, id);
            crate::systems::gossip::exchange(world, partner, id, crate::systems::gossip::Venue::Chat);
            world.release_all(id);
            StepResult::Done
        }
        ActionKind::Flirt => {
            let Some(partner) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            if world.comp::<Position>(id).and_then(|p| p.building)
                != world.comp::<Position>(partner).and_then(|p| p.building)
            {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            let aff = world.edge(id, partner).map_or(0.0, |e| e.affinity);
            let sociable = world.comp::<crate::components::Personality>(partner).map_or(0.5, |p| p.sociability);
            let accept = {
                use rand::Rng;
                let roll: f32 = world.rng.world().random();
                !crate::systems::social::has_spouse(world, partner)
                    && !world.rejected_recently(id, partner)
                    && roll < 0.3 + aff + 0.2 * sociable
            };
            if accept {
                for who in [id, partner] {
                    if let Some(n) = world.comp_mut::<Needs>(who) {
                        n.intimacy = (n.intimacy + 0.1).min(1.0);
                    }
                    world.remember(
                        who,
                        MemoryKind::Courted,
                        Some(if who == id { partner } else { id }),
                        0.4,
                        0.3,
                        false,
                    );
                }
                crate::systems::social::adjust(world, id, partner, 0.1, 0.02);
            } else {
                world.remember(id, MemoryKind::Rejected, Some(partner), 0.4, -0.3, false);
            }
            world.release_all(id);
            StepResult::Done
        }
        ActionKind::Propose => {
            let Some(partner) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            if world.comp::<Position>(id).and_then(|p| p.building)
                != world.comp::<Position>(partner).and_then(|p| p.building)
            {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            crate::systems::social::propose(world, id, partner);
            world.release_all(id);
            StepResult::Done
        }
        ActionKind::JoinGang => {
            if crate::systems::gang::join(world, id) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::PreconditionLost)
            }
        }
        ActionKind::Extort => {
            let Some(home) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            crate::systems::gang::extort(world, id, home);
            StepResult::Done
        }
        ActionKind::SplitLoot => {
            crate::systems::gang::split_loot(world, id);
            StepResult::Done
        }
        ActionKind::Fence => {
            // M13 D26: a stolen vehicle first, else stolen food.
            if crate::systems::vehicles::stolen_held_by(world, id).is_some() {
                return if crate::systems::vehicles::fence(world, id) {
                    StepResult::Done
                } else {
                    StepResult::Failed(FailReason::StockGone)
                };
            }
            if crate::systems::gang::fence(world, id) > 0 {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        // M13 D29: the pick is bought here (price, finance, import); D34: an
        // implant is installed in the same action.
        ActionKind::BuyAsset => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let pick = world.comp_mut::<Brain>(id).and_then(|b| b.shop_pick.take());
            match (here, pick) {
                (Some(seller), Some(pick)) if target == Some(seller) => {
                    match crate::systems::chrome::buy_install(world, id, seller, &pick) {
                        Ok(_) => StepResult::Done,
                        Err(_) => StepResult::Failed(FailReason::PreconditionLost),
                    }
                }
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        // M14 V10/V12: the run starts; the body sits `JackedIn` until it ends.
        ActionKind::JackIn => match crate::systems::virt::start_run(world, id) {
            Ok(run) => {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.exec = crate::exec::ExecState::JackedIn { run, since: now };
                }
                StepResult::Running
            }
            Err(reason) => StepResult::Failed(reason),
        },
        // M14 V17/V29: the deck's Data to the buyer here.
        ActionKind::SellData => {
            if crate::systems::tech::sell_deck_data(world, id) > 0 {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        // M14 V36: the carried deck one tier up.
        ActionKind::UpgradeDeck => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let pick = world.comp_mut::<Brain>(id).and_then(|b| b.shop_pick.take());
            match (here, pick) {
                (Some(seller), Some(pick)) if target == Some(seller) && pick.upgrade => {
                    match crate::systems::assets::upgrade_deck(world, id, seller, &pick) {
                        Ok(()) => StepResult::Done,
                        Err(_) => StepResult::Failed(FailReason::PreconditionLost),
                    }
                }
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        // M13 D34: the gang's implant, for the fee.
        ActionKind::Install => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let implant = crate::systems::chrome::reserved_implant(world, id);
            let tier = implant.and_then(|a| world.comp::<crate::components::Asset>(a)).map_or(1, |x| x.tier);
            let fee = crate::systems::chrome::install_fee(world, tier);
            world.comp_mut::<Brain>(id).map(|b| b.shop_pick.take());
            match (here, implant) {
                (Some(c), Some(a)) if crate::systems::chrome::install(world, id, c, a, fee).is_ok() => {
                    if let Some(b) = world.comp_mut::<crate::components::Body>(id) {
                        b.last_shop = Some(now);
                    }
                    StepResult::Done
                }
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        ActionKind::Therapy => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            match here {
                Some(c) if crate::systems::chrome::therapy(world, id, c) => StepResult::Done,
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        // M13 D38: a batch from the Hideout.
        ActionKind::PickUp => {
            if crate::systems::stims::pick_up(world, id) > 0 {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        // M13 D38: the shift is over; the gang's task is done for the day.
        ActionKind::Deal => {
            crate::systems::stims::end_deal(world, id);
            // L2 (L25): a deal shift is the order's act on the ledger.
            crate::systems::fviolence::note_act(world, id, crate::ledger::ActKind::Deal);
            let today = world.day();
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.gang_task_day = Some(today);
            }
            StepResult::Done
        }
        // M13 D38/D40: doses from the dealer or the legal Market here.
        ActionKind::BuyStims => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            match here {
                Some(b) if crate::systems::stims::buy_stims(world, id, b) > 0 => StepResult::Done,
                _ => StepResult::Failed(FailReason::StockGone),
            }
        }
        // M13 D39: a dose (the overdose roll may end the agent here).
        ActionKind::UseStim => {
            if crate::systems::stims::use_stim(world, id) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        // M13 D39: Detox at the Clinic.
        ActionKind::Detox => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            match here {
                Some(c) if crate::systems::stims::detox(world, id, c) => StepResult::Done,
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        ActionKind::Uninstall => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            match here {
                Some(c) if crate::systems::chrome::uninstall(world, id, c) => StepResult::Done,
                _ => StepResult::Failed(FailReason::PreconditionLost),
            }
        }
        // M13 D35: a body with chrome left is ripped next by one who may.
        ActionKind::Strip => {
            let Some(c) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            // M15 W35: a guard standing over the body fights for it first.
            if !crate::systems::grudges::guard_contests(world, id, c) {
                return StepResult::Failed(FailReason::PreconditionLost);
            }
            if !crate::systems::chrome::strip(world, id, c) {
                return StepResult::Failed(FailReason::StockGone);
            }
            if crate::systems::chrome::may_rip(world, id) && !crate::systems::chrome::installed(world, c).is_empty() {
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    let at = usize::from(b.plan_step) + 1;
                    if let Some(plan) = b.plan.as_mut() {
                        let at = at.min(plan.steps.len());
                        plan.steps.insert(
                            at,
                            crate::components::ActionInstance { action: ActionKind::Rip, target: Some(c), tile: None },
                        );
                    }
                }
            }
            StepResult::Done
        }
        ActionKind::Rip => {
            let Some(b) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            // M15 W35: a guard standing over the body fights for it first.
            if !crate::systems::grudges::guard_contests(world, id, b) {
                return StepResult::Failed(FailReason::PreconditionLost);
            }
            let live = crate::systems::law::living(world, b);
            let n = crate::systems::chrome::rip(world, id, b);
            // A Harvest crew's job is done once the take is home.
            if world.has::<crate::components::GangMember>(id) {
                let today = world.day();
                if let Some(x) = world.comp_mut::<Brain>(id) {
                    x.gang_task_day = Some(today);
                }
            }
            if n == 0 && !live {
                StepResult::Failed(FailReason::StockGone)
            } else {
                StepResult::Done
            }
        }
        ActionKind::Abduct => {
            let Some(v) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if crate::systems::chrome::abduct(world, id, v) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::PreconditionLost)
            }
        }
        // M13 D26: the lock contest; a loss is a botched theft.
        ActionKind::StealVehicle => {
            let Some(v) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if crate::systems::vehicles::steal(world, id, v) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::PreconditionLost)
            }
        }
        ActionKind::Muster => {
            if crate::systems::raid::depart(world, id) {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::PreconditionLost)
            }
        }
        ActionKind::Brawl => {
            // M12 phase 4: the first at the door waits for the crew.
            if crate::systems::raid::wait_for_crew(world, id) {
                return StepResult::Running;
            }
            crate::systems::raid::resolve(world, id);
            StepResult::Done
        }
        ActionKind::Attack => {
            let Some(victim) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            if !crate::systems::law::living(world, victim) || !crate::systems::law::near(world, id, victim, 4) {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            // M16a (plan C32): a Guard taker on post beside the client meets
            // the attack first; a guard's win ends it (the attacker's Assault).
            if let Some(g) = crate::systems::contracts::guard_for(world, id, victim) {
                let (winner, _, died) = crate::systems::law::resolve_fight(world, id, g);
                let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
                let text =
                    format!("{} attacked {}'s guard {}", world.name_of(id), world.name_of(victim), world.name_of(g));
                world.push_event(crate::events::EventKind::Assault, &[id, g], text);
                if crate::systems::law::living(world, id) {
                    let murder = died && winner == id;
                    let crime =
                        if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
                    crate::systems::law::raise_crime_on(world, id, (!murder).then_some(g), Some(g), crime, tile);
                }
                if winner != id || !crate::systems::law::living(world, victim) {
                    crate::systems::hunt::on_strike(world, id, victim, g, false);
                    return StepResult::Done;
                }
            }
            // M13 D33: a berserker's blows kill at `berserk_kill_mult`.
            // M15 W22: a lethal Hunt's strike kills at `hunt_kill_p`.
            let mods = crate::systems::law::FightMods {
                kill_mult: crate::systems::chrome::attack_kill_mult(world, id),
                a_bonus: 0.0,
                kill_p: crate::systems::hunt::strike_kill_p(world, id, victim),
            };
            let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, id, victim, mods);
            // The attacker's crime: Murder only when the victim died. An attacker
            // who died resisting is charged with nothing (the dead cannot be).
            let murder = died && loser == victim;
            let crime = if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
            let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
            let name = world.name_of(id);
            let text = if died && loser == id {
                format!("{name} attacked {} and died", world.name_of(victim))
            } else {
                format!("{name} attacked {}", world.name_of(victim))
            };
            world.push_event(
                if murder { crate::events::EventKind::Murder } else { crate::events::EventKind::Assault },
                &[id, victim],
                text,
            );
            if crate::systems::law::living(world, id) {
                crate::systems::law::raise_crime_on(world, id, (!murder).then_some(victim), Some(victim), crime, tile);
            }
            // M15 W22: the Hunt's strike (after the crime: street silence reads the Hunt).
            crate::systems::hunt::on_strike(world, id, victim, winner, died);
            StepResult::Done
        }
        // M15 W21: the answer to the Hunt's question.
        ActionKind::AskAround => crate::systems::hunt::ask_around(world, id),
        // M15 W22: contact (Done, the Attack next) or the timeout; W35: the
        // guard's wait is over (cooled a day: the body is not stood over
        // around the clock).
        ActionKind::StakeOut => {
            let guard =
                world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::GuardBody);
            if guard {
                if let Some(c) = target {
                    crate::systems::grudges::end_guard(world, id, c);
                }
                let until = world.tick + crate::time::TICKS_PER_DAY;
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.cooldowns.insert(crate::components::GoalKind::GuardBody, until);
                }
                StepResult::Done
            } else if crate::systems::hunt::contact(world, id) {
                // M16a (plan C27): a Locate tracker's contact is a paid sighting.
                crate::systems::contracts::on_contact(world, id);
                StepResult::Done
            } else {
                crate::systems::hunt::stakeout_failed(world, id)
            }
        }
        ActionKind::CarryCorpse => {
            let Some(c) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if !world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried) {
                return StepResult::Failed(FailReason::StockGone);
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.carrying_corpse = Some(c);
            }
            world.remember(id, MemoryKind::SawCorpse, Some(c), 0.5, -0.4, false);
            StepResult::Done
        }
        ActionKind::BuryCorpse => {
            let Some(c) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if crate::systems::demography::bury(world, id, c) {
                world.release_all(id);
                // L2 shadow fixes item 22: the burial is the digger's shift.
                crate::systems::fixes::burial_shift(world, id);
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
            }
        }
        ActionKind::StealFood(source) => steal_food(world, id, source, target),
        ActionKind::Forage => {
            if let Some(inv) = world.comp_mut::<Inventory>(id) {
                inv.food = (inv.food + 1).min(20);
            }
            StepResult::Done
        }
        ActionKind::Beg => beg(world, id),
        ActionKind::SellFood => {
            let market = world.local(id, BuildingKind::Market);
            let price = market.map_or(0, |m| world.price_at(m));
            let pay = (price as f32 * 0.6).floor() as i64;
            // M11 D46: the Market's owner buys (refused when it cannot pay).
            let owner = market.and_then(|m| world.owner_of(m));
            let purse = world.purse(owner);
            let Some(inv) = world.comp::<Inventory>(id) else { return StepResult::Failed(FailReason::StockGone) };
            let units = inv.food.saturating_sub(inv.stolen_food).min(3);
            if units == 0 || purse < pay * i64::from(units) {
                return StepResult::Failed(FailReason::StockGone);
            }
            if let Some(inv) = world.comp_mut::<Inventory>(id) {
                inv.food -= units;
            }
            if let Some(m) = market.and_then(|m| world.comp_mut::<Building>(m)) {
                m.stock_food += units;
            }
            crate::systems::ownership::pay(
                world,
                owner,
                Some(id),
                pay * i64::from(units),
                crate::systems::ownership::Flow::SellFood,
            );
            world.remember(id, MemoryKind::Paid, None, 0.1, 0.1, false);
            StepResult::Done
        }
        _ => StepResult::Failed(FailReason::PreconditionLost),
    }
}

/// Theft: units leave the source into the thief's inventory as stolen food.
/// Witnesses, crime reports and arrests arrive with the law system (M4); for
/// now the event and the daily counter record it.
fn steal_food(world: &mut World, id: EntityId, source: StealSource, target: Option<EntityId>) -> StepResult {
    let (building, take) = match source {
        StealSource::Market => (world.local(id, BuildingKind::Market), 2),
        StealSource::Home => (target, 2),
        StealSource::Warehouse => (world.building_of_kind(BuildingKind::Warehouse), 5),
    };
    let Some(b) = building else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let Some(bd) = world.comp_mut::<Building>(b) else { return StepResult::Failed(FailReason::NoSuchPlace) };
    let units = take.min(bd.stock_food);
    if units == 0 {
        return StepResult::Failed(FailReason::StockGone);
    }
    bd.stock_food -= units;
    let kind = bd.kind;
    if let Some(inv) = world.comp_mut::<Inventory>(id) {
        inv.food = (inv.food + units).min(20);
        inv.stolen_food = (inv.stolen_food + units).min(inv.food);
    }
    world.stats.current.thefts += 1;
    // M11 D20: a corp-owned Market books the loss at its price.
    if kind == BuildingKind::Market {
        let coins = i64::from(units) * world.price_at(b);
        crate::systems::ownership::note_loss(world, b, coins, Some(id));
    }
    let name = world.name_of(id);
    world.push_event(
        crate::events::EventKind::Theft,
        &[id, b],
        format!("{name} stole {units} food from the {}#{}", kind.label(), b.index),
    );
    world.release_all(id);
    // Witnesses: the victims are the building's occupants (a Home's residents,
    // the clerk on duty); for the Market and Warehouse there is no single victim.
    let tile = world.comp::<Position>(id).map_or(TilePos::default(), |p| p.tile);
    let victim = match source {
        StealSource::Home => world.comp::<Building>(b).and_then(|bd| bd.occupants.iter().copied().find(|&o| o != id)),
        _ => None,
    };
    crate::systems::law::raise_crime(world, id, victim, crate::components::Crime::Theft, tile);
    StepResult::Done
}

/// Beg: up to 4 passers-by within 3 tiles (lowest id first); each with
/// `edge.affinity > 0` and `coins > 10` rolls `rng < 0.3 + sociability × 0.4`;
/// the first success gives `1 + (rng < 0.5)` coins and records a debt. Fails
/// (memory Rejected) if nothing was gained.
fn beg(world: &mut World, id: EntityId) -> StepResult {
    use rand::Rng;
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else {
        return StepResult::Failed(FailReason::NoSuchPlace);
    };
    let sociability = world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.sociability);
    let passers: Vec<EntityId> = world
        // scan-ok: per Beg, includes Statistical passers
        .citizens()
        .into_iter()
        .filter(|&o| o != id)
        .filter(|&o| world.comp::<Position>(o).is_some_and(|p| p.tile.manhattan(tile) <= 3))
        .take(4)
        .collect();
    // L1: a stranger may give too (the broke begged only from friends, and
    // the street is strangers), at half the odds.
    let strangers = world.config.life.enabled;
    for other in passers {
        let key = crate::components::edge_key(id, other);
        let edge = world.edges.get(&key).map(|e| e.affinity);
        let generous = (edge.is_some_and(|a| a > 0.0) || (strangers && edge.is_none()))
            && world.comp::<Wallet>(other).is_some_and(|w| w.coins > 10);
        if !generous {
            continue;
        }
        let roll: f32 = world.rng.world().random();
        let odds = (0.3 + sociability * 0.4) * if edge.is_none() { 0.5 } else { 1.0 };
        if roll < odds {
            let bonus: f32 = world.rng.world().random();
            let coins = 1 + i64::from(bonus < 0.5);
            if let Some(w) = world.comp_mut::<Wallet>(other) {
                w.coins -= coins;
            }
            if let Some(w) = world.comp_mut::<Wallet>(id) {
                w.coins += coins;
            }
            {
                // the lower id owes the higher id
                let (lo, _) = key;
                let tick = world.tick;
                let e = world.edge_entry(id, other);
                e.debt += if lo == id { coins as i32 } else { -(coins as i32) };
                e.debt_since.get_or_insert(tick);
                e.last_interaction = tick;
            }
            return StepResult::Done;
        }
    }
    world.remember(id, MemoryKind::Rejected, None, 0.3, -0.3, false);
    StepResult::Failed(FailReason::PreconditionLost)
}

/// Walks with a completion effect: Escort delivers the suspect, FleeToHome
/// settles the nerves.
pub fn on_arrive(world: &mut World, id: EntityId, step: &crate::components::ActionInstance) {
    match step.action {
        ActionKind::Escort => {
            let suspect = world.comp::<Brain>(id).and_then(|b| b.escorting).or(step.target);
            if let Some(s) = suspect {
                crate::systems::law::jail_suspect(world, id, s);
            }
        }
        ActionKind::FleeToHome => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.safety = n.safety.max(0.5);
            }
            // L2 shadow fixes item 6: the flight holds a while: no leisure
            // or company out on the street for `flee_clear_ticks`.
            if crate::systems::fixes::item(world, 6) {
                let until = world.tick + world.config.life.flee_clear_ticks;
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    for g in [crate::components::GoalKind::Unwind, crate::components::GoalKind::Socialise] {
                        let c = b.cooldowns.entry(g).or_insert(0);
                        *c = (*c).max(until);
                    }
                }
            }
        }
        // L1: at the last-seen tile, chase a suspect still in sight; a
        // sighting gone stale is dropped (the Arrest step then fails).
        ActionKind::GoTo(crate::goap::LocationKey::SuspectTile) if world.config.life.enabled => {
            if let Some(s) = step.target {
                crate::systems::life::arrive_at_suspect(world, id, s);
            }
        }
        _ => {}
    }
}

/// `ReportCrime` at the Hall: file the most salient unreported first-hand
/// SawCrime as a report.
fn report_crime(world: &mut World, id: EntityId) -> StepResult {
    let betraying = world.comp::<Brain>(id).is_some_and(|b| b.betraying);
    let leader = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)).and_then(|g| g.leader);
    let Some(mem) = world.comp::<crate::components::Memory>(id) else {
        return StepResult::Failed(FailReason::PreconditionLost);
    };
    let best = mem
        .entries
        .iter()
        .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.salience >= 0.5)
        .filter(|e| e.subject.is_some_and(|s| !crate::systems::law::reported_since(world, s, e.tick)))
        // A betrayer names the leader's most salient crime first.
        .max_by(|a, b| {
            (betraying && a.subject == leader)
                .cmp(&(betraying && b.subject == leader))
                .then(a.salience.total_cmp(&b.salience))
        })
        .map(|e| (e.subject, e.crime));
    let Some((Some(suspect), crime)) = best else { return StepResult::Failed(FailReason::PreconditionLost) };
    let crime = crime.unwrap_or(crate::components::Crime::Theft);
    crate::systems::law::file_report(world, crime, suspect, Some(id));
    if let Some(p) = world.comp_mut::<crate::components::Personality>(id) {
        p.drift(crate::personality::Drift::ReportedCrime);
    }
    if betraying && leader == Some(suspect) {
        crate::systems::gang::betrayal_filed(world, id);
    }
    StepResult::Done
}

/// Shift end: the shift is marked worked (so Work stays pending while it
/// runs and a resumed shift is not counted twice) and a day of wages is owed.
/// A guard's wages are owed by the shift clock instead
/// (`law::credit_guard_shifts`), whichever plan the guard ends the shift on.
pub fn end_shift(world: &mut World, id: EntityId) {
    let tick = world.tick;
    let mut owed = false;
    if let Some(j) = world.comp_mut::<Job>(id) {
        j.last_shift_day = Some(j.shift_key_at(tick.saturating_sub(1)));
        if j.role != crate::components::Role::Guard {
            j.days_unpaid = j.days_unpaid.saturating_add(1);
            owed = true;
        }
    }
    // L1: paid at the workplace as the shift ends (as the Statistical tier
    // always was), not on a walk to the Hall; a short payment carries over
    // to the next shift's.
    if owed && world.config.life.enabled {
        economy::collect_wage(world, id);
        return;
    }
    economy::maybe_quit(world, id);
}

/// L1 `Scavenge`: `[life] scavenge_coins` from the Treasury (the Recycler's
/// scrap price, `Flow::Sanitation`) when it is not negative.
fn scavenge(world: &mut World, id: EntityId) -> StepResult {
    use rand::Rng;
    // An hour turns up something worth selling on `scavenge_p` of tries
    // (the agent's keyed stream).
    // L2 L9: with jobs on, the district's litter scales the find (the
    // same draw on the same stream).
    let p = crate::systems::jobs::scavenge_p(world, id);
    let found = world.rng.agent(id).random::<f32>() < p;
    let treasury = world.treasury().map_or(0, |t| t.coins);
    let pay = world.config.life.scavenge_coins.min(treasury.max(0));
    if pay <= 0 || !found {
        // L2 (L29, `[lod] budget`): a dry hour is an hour honestly spent,
        // not an abort; `scavenge_dry_max` of them in a row cool Earn.
        if crate::systems::lod::budget_on(world) {
            let now = world.tick;
            let max = world.config.life.scavenge_dry_max.max(1);
            let cool = now + world.config.life.scavenge_cool_hours * crate::time::TICKS_PER_HOUR;
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.scavenge_dry = b.scavenge_dry.saturating_add(1);
                if b.scavenge_dry >= max {
                    b.scavenge_dry = 0;
                    b.cooldowns.insert(crate::components::GoalKind::Earn, cool);
                }
            }
            world.stats.current.budget.scavenge_dry += 1;
            return StepResult::Done;
        }
        return StepResult::Failed(FailReason::StockGone);
    }
    // L2 shadow fixes item 21: booked as Scavenge (it read "Sanitation +1
    // from the City" in the diaries).
    let flow = if crate::systems::fixes::item(world, 21) {
        crate::systems::ownership::Flow::Scavenge
    } else {
        crate::systems::ownership::Flow::Sanitation
    };
    crate::systems::ownership::pay(world, None, Some(id), pay, flow);
    world.remember(id, MemoryKind::Paid, None, 0.1, 0.0, false);
    // L2 L9: the find is scrap for the Recycler's Parts.
    crate::systems::jobs::add_scrap(world);
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.scavenge_dry = 0;
    }
    StepResult::Done
}

/// Sleeping at home with a spouse who lives there too. Checked by household
/// rather than by the spouse's position at wake-up, because one of them has
/// usually left for a shift by then.
fn spouse_in_same_home(world: &World, id: EntityId) -> bool {
    let Some(here) = world.comp::<Position>(id).and_then(|p| p.building) else { return false };
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    home == Some(here) && world.spouse_of(id).is_some_and(|s| world.comp::<Household>(s).and_then(|h| h.home) == home)
}
