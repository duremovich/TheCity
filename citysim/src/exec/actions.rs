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
        ActionKind::Muster => world
            .gang_of(id)
            .and_then(|g| world.comp::<crate::components::Gang>(g))
            .and_then(|g| g.raid_at)
            .map_or(0, |t| t.saturating_sub(world.tick)),
        ActionKind::Brawl => 15,
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
        ActionKind::BuyFood => {
            at(world, id, BuildingKind::Market)
                && economy::buy_quantity(world, id, world.local(id, BuildingKind::Market)) > 0
        }
        ActionKind::Sleep | ActionKind::Rest => {
            // Sleep on the street is allowed (homeless); Rest anywhere indoors.
            true
        }
        ActionKind::Drink => at(world, id, BuildingKind::Bar) && coins >= 2,
        // M11 D13: at the wage desk (the Hall for a city job, else the workplace).
        ActionKind::CollectWage => {
            world.comp::<Position>(id).is_some_and(|p| p.building.is_some() && p.building == world.wage_desk(id))
                && world.comp::<Job>(id).is_some_and(|j| j.wage_collectable(world.day()))
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
            world.pending_purchase.insert(id, (units, paid));
        }
        ActionKind::Wander => {
            // Effect `at = Street`: step out of whatever building we are in.
            world.leave_building(id);
        }
        ActionKind::HaulToMarket => {
            // The transfer happens on pickup so an interrupted walk cannot lose the load.
            if let Some(farm) = target {
                economy::haul(world, farm);
            }
        }
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
    match kind {
        ActionKind::FarmWork => {
            if let Some(farm) = world.comp::<Job>(id).and_then(|j| j.employer) {
                let now = world.tick;
                economy::accrue_farm_work(world, id, farm, now.saturating_sub(started));
            }
        }
        ActionKind::BuyFood => {
            if let Some((_, paid)) = world.pending_purchase.remove(&id) {
                let market = world.local(id, BuildingKind::Market);
                economy::refund_food(world, id, market, paid);
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
        ActionKind::Rest => {
            crate::exec::routine::must_leave_for_work(world, id)
                || world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j))
        }
        // Waiting at the workplace for the shift: start the moment it begins.
        ActionKind::Wander => world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j)),
        // The order changed: the muster breaks up (on_complete then fails the step).
        ActionKind::Muster => crate::systems::raid::raid_done(world, id),
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
            let hideout_home = here.is_some() && crate::systems::gang::holes_up_at(world, id) == here;
            if at_home(world, id) || hideout_home {
                world.mark_day(id, crate::components::trace_flags::SLEPT_AT_HOME);
                // M10: +0.2 / +0.3 / +0.4 by the building's tier (Sump, Mid, Spire).
                let tier = here.and_then(|b| world.comp::<Building>(b)).map_or(1, |b| b.tier);
                if let Some(n) = world.comp_mut::<Needs>(id) {
                    n.safety = (n.safety + 0.2 + 0.1 * f32::from(tier)).min(1.0);
                }
                // (the spouse's intimacy bonus is granted at Sleep start: most
                // nights' sleep is cut short by the morning shift, not completed)
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
            crate::systems::social::gossip(world, id, partner);
            crate::systems::social::gossip(world, partner, id);
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
            if crate::systems::gang::fence(world, id) > 0 {
                StepResult::Done
            } else {
                StepResult::Failed(FailReason::StockGone)
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
            crate::systems::raid::resolve(world, id);
            StepResult::Done
        }
        ActionKind::Attack => {
            let Some(victim) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            if !crate::systems::law::living(world, victim) || !crate::systems::law::near(world, id, victim, 4) {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            let (_, loser, died) = crate::systems::law::resolve_fight(world, id, victim);
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
                crate::systems::law::raise_crime(world, id, (!murder).then_some(victim), crime, tile);
            }
            StepResult::Done
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
    for other in passers {
        let key = crate::components::edge_key(id, other);
        let generous = world.edges.get(&key).is_some_and(|e| e.affinity > 0.0)
            && world.comp::<Wallet>(other).is_some_and(|w| w.coins > 10);
        if !generous {
            continue;
        }
        let roll: f32 = world.rng.world().random();
        if roll < 0.3 + sociability * 0.4 {
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
    if let Some(j) = world.comp_mut::<Job>(id) {
        j.last_shift_day = Some(j.shift_key_at(tick.saturating_sub(1)));
        if j.role != crate::components::Role::Guard {
            j.days_unpaid = j.days_unpaid.saturating_add(1);
        }
    }
    economy::maybe_quit(world, id);
}

/// Sleeping at home with a spouse who lives there too. Checked by household
/// rather than by the spouse's position at wake-up, because one of them has
/// usually left for a shift by then.
fn spouse_in_same_home(world: &World, id: EntityId) -> bool {
    let Some(here) = world.comp::<Position>(id).and_then(|p| p.building) else { return false };
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    home == Some(here) && world.spouse_of(id).is_some_and(|s| world.comp::<Household>(s).and_then(|h| h.home) == home)
}
