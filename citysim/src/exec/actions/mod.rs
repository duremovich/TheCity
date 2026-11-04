//! Per-action preconditions, durations, start effects (money) and
//! completion effects (everything else). Only the actions the M1 routine
//! uses are implemented; the rest fail `can_start` until their milestone.

use crate::components::{
    AssetKind, Brain, Building, BuildingKind, Household, Inventory, Job, MemoryKind, Needs, Position, Skills, TilePos,
    Wallet,
};
use crate::entity::EntityId;
use crate::exec::{FailReason, StepResult};
use crate::goap::{ActionKind, StealSource};
use crate::needs;
use crate::systems::economy;
use crate::time::Tick;
use crate::world::World;

mod complete;
mod outcomes;

pub use complete::*;
pub use outcomes::*;

/// Fixed durations in ticks from the Action table; work actions run to the
/// shift end and Sleep is capped by `sleep_ticks_full`.
pub fn duration(world: &World, id: EntityId, kind: ActionKind) -> Tick {
    match kind {
        ActionKind::EatFromInventory => 10,
        ActionKind::EatAtHome => 15,
        ActionKind::BuyFood => 10,
        ActionKind::Sleep => {
            let full = Tick::from(world.config.needs.sleep_ticks_full);
            // Workplace room: a housed agent sleeping rough sleeps on (the
            // committed sleep's own ends, `finishes_early`, wake it: rested,
            // or the walk to the shift); woken at the cap it walked home at
            // 02:00 across the city to sleep again.
            let rough = world.config.life.rough_sleep_on
                && world.comp::<crate::components::Household>(id).is_some_and(|h| h.home.is_some())
                && world.comp::<crate::components::Position>(id).is_some_and(|p| p.building.is_none());
            if rough {
                full * 2
            } else {
                full
            }
        }
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
        ActionKind::Fence => 15,
        // C4: the swing comes at once; the fight itself is the live engagement
        // the step holds Running (C3a). The old 15 was a one-roll fight's.
        ActionKind::Attack => 1,
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
        // Round 7 W2 (OPEN-241): on the corner he was called to, until the fight.
        ActionKind::HangOut => {
            let base = world.config.leisure.hangout_ticks;
            crate::systems::turf::muster_wait(world, id).map_or(base, |w| w.max(base))
        }
        // Lives at work phase 4b (§ 2.3): a pimp's collection is the hour at the pitch.
        ActionKind::Collect => crate::systems::vice::collect_ticks(world, id).unwrap_or(10),
        // M16a (plan C10, C15).
        ActionKind::Network => 30,
        // Round 9 Z2: the walk-in's minutes at the desk.
        ActionKind::TakeJob => world.config.fixers.take_job_mins,
        // The Hit market (H9).
        ActionKind::Commission => world.config.hitmarket.commission_mins,
        // M18 P5h: the application at the desk.
        ActionKind::Apply => world.config.housing.apply_mins,
        ActionKind::HandOver => 5,
        // M16b phase 3 (spec § 5: Stabilise is dur 30).
        ActionKind::CarryWounded => 5,
        ActionKind::Stabilise => 30,
        // Real economy E27, E28: a meal at the kitchen, a gift at the door.
        ActionKind::EatAlms => 30,
        ActionKind::DumpsterDive => 20,
        ActionKind::Oncology => 120,
        ActionKind::Patch => crate::systems::customers::patch_ticks(world),
        ActionKind::PackMeal => 5,
        // Jobs & lives phase 2: an hour at the Pitch, or to the dawn pick.
        ActionKind::Vend => crate::systems::streetwork::vend_ticks(world, id),
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
            let left = world
                .comp::<Job>(id)
                .and_then(|j| j.shift_end(world.tick))
                .map_or(0, |end| end.saturating_sub(world.tick));
            // The pre-view pass phase 2 (OPEN-137): a stations block; phase 4
            // (P8): a stop of the sweeper's round.
            crate::systems::shift::block_ticks(world, id)
                .or_else(|| crate::systems::jobs::sweep_stop_ticks(world, id))
                .map_or(left, |b| b.min(left))
        }
        // The pre-view pass phase 2 (P7): the break, to `[shift] break_min`.
        ActionKind::Break => crate::systems::shift::break_ticks(world, id),
        // The pre-view pass phase 3: a haul trip's unload, `[shift.haul] haul_handle_min`.
        ActionKind::Unload => crate::systems::shift::unload_ticks(world),
        // The pre-view pass phase 6 (P14, P15): court to its end, or an order given.
        ActionKind::Court => crate::systems::leaders::court_ticks(world, id),
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
    // Round 4 T1 (OPEN-231): the adults' share (the children's food kept
    // back on the service's warning).
    world.comp::<Household>(id).and_then(|h| h.home).map_or(0, |h| crate::systems::camp::adults_share(world, id, h))
}

/// On shift, at the employer, and this shift not yet worked.
pub fn can_work_now(world: &World, id: EntityId, job: &Job) -> bool {
    job.on_shift(world.tick_of_day())
        && job.last_shift_day != Some(job.shift_key_at(world.tick))
        && if job.role == crate::components::Role::Sanitation && crate::systems::jobs::sweep_on(world) {
            // L2 L8: a sweeper works out on its beat's streets; the pre-view
            // pass phase 4 (P8): his round closes at the Recycler (the post).
            crate::systems::jobs::on_beat(world, id) || crate::systems::jobs::on_round(world, id, job)
        } else {
            job.employer.is_some_and(|e| world.comp::<Position>(id).is_some_and(|p| p.building == Some(e)))
                // The island pass phase 8: a Builder's site day is worked at the site.
                || crate::systems::shift::on_site(world, id, job)
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
        // Hunger as sickness § 4: the dive's start check.
        ActionKind::DumpsterDive => crate::systems::bins::can_start(world, id, target),
        ActionKind::Oncology => crate::systems::health::can_start_oncology(world, id, target),
        ActionKind::Patch => crate::systems::customers::can_patch(world, id, target),
        ActionKind::PackMeal => at_home(world, id) && home_pantry(world, id) > 0,
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
        // M18 P2: an ask at a bound door (a directed plan's; the NPC's
        // Pitch Vend is unbound and checked by `streetwork::can_start`).
        ActionKind::Vend => target.is_some_and(|b| crate::systems::search::at_door(world, id, b)),
        // Pressure pass round 9, Y2 (`OPEN-250`): a swing across the cells'
        // wall does not start (the planner's foe taken in mid-plan).
        ActionKind::Attack => target.is_none_or(|t| !crate::systems::law::held_apart(world, id, t)),
        // The pre-view pass phase 2 (P7): the break is outside the door.
        ActionKind::Break => crate::systems::shift::can_break(world, id),
        // The pre-view pass phase 3: inside the trip's destination.
        ActionKind::Unload => target.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == target,
        // The pre-view pass phase 6: at the Hideout in court hours, or at the lieutenant.
        ActionKind::Court => crate::systems::leaders::can_court(world, id, target),
        // L1: out on the street; office hours inside the HQ.
        // R5 (OPEN-214): or inside a building the planner reads as the
        // street (someone else's Home after an Extort, a back room): the
        // planner only plans it there; `on_start` steps out. Never at home.
        ActionKind::Scavenge => world
            .comp::<Position>(id)
            .is_some_and(|p| p.building.is_none_or(|b| world.comp::<Household>(id).and_then(|h| h.home) != Some(b))),
        ActionKind::Meeting => target.is_some() && world.comp::<Position>(id).and_then(|p| p.building) == target,
        // The pressure pass F1: at the Hall or a Station's desk.
        ActionKind::Register => {
            crate::systems::founding::at_registry(world, id) && crate::systems::founding::can_found(world, id)
        }
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
            // Jobs & lives phase 3: a weapon's seller is a Gunsmith or a dealer's Bar.
            let weapon =
                world.comp::<Brain>(id).and_then(|b| b.shop_pick.as_ref()).is_some_and(|p| p.kind == AssetKind::Weapon);
            world.comp::<Position>(id).and_then(|p| p.building).is_some_and(|b| {
                Some(b) == target
                    && !world.is_closed(b)
                    && if weapon {
                        crate::systems::weapons::sale_open(world, b)
                    } else {
                        crate::systems::assets::seller_open(world, b)
                    }
            }) && world.comp::<Brain>(id).and_then(|b| b.shop_pick.as_ref()).is_some_and(|p| {
                // OPEN-142: the sale is checked before the wait (an implant's
                // is 140 min on the table), not only at its end.
                weapon || target.is_some_and(|t| crate::systems::assets::sale_ok(world, id, t, p))
            })
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
        // C8: in any of its Hideouts, the stash the seat's (the cribs draw on it).
        ActionKind::PickUp => {
            let gang = world.gang_of(id);
            let hideout = gang.and_then(|g| world.hideout_of(g));
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            hideout.is_some()
                && here.is_some_and(|b| gang.is_some_and(|g| crate::systems::vertical::is_hideout_of(world, g, b)))
                && hideout.is_some_and(|h| world.stock(h, crate::components::Good::Stims) > 0)
        }
        // M13 D38: inside the bound deal Bar with doses to sell.
        // Lives at work phase 4b (§ 2.3): no target, the dealer's corner.
        ActionKind::Deal if target.is_none() => crate::systems::stims::corner_deal_ok(world, id),
        ActionKind::Deal => {
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            here.is_some() && here == target && !closed_here() && inv.is_some_and(|i| i.stims > 0)
        }
        ActionKind::BuyStims if target.is_none() => crate::systems::stims::corner_buy_ok(world, id),
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
            world.run_orders.get(&id).is_some_and(|o| Some(o.chair) == here && world.tick >= o.not_before)
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
            let here = world.comp::<Position>(id).and_then(|p| p.building);
            let at_home = here.is_some_and(|b| {
                world.gang_of(id).is_some_and(|g| crate::systems::vertical::is_hideout_of(world, g, b))
            });
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
        // R5 (OPEN-214): a scripted try-out stands inside its place (a
        // Hideout or a front); the planner's symbols cover the rest.
        ActionKind::JoinGang => {
            !world.has::<crate::components::GangMember>(id)
                && target.is_none_or(|t| world.comp::<Position>(id).and_then(|p| p.building) == Some(t))
        }
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
        | ActionKind::Extort
        | ActionKind::SplitLoot
        | ActionKind::Fence
        | ActionKind::Muster
        | ActionKind::Brawl => true,
        _ => false,
    }
}

/// Start effects: money moves now so an interruption cannot duplicate it.
pub fn on_start(world: &mut World, id: EntityId, kind: ActionKind, target: Option<EntityId>) {
    match kind {
        // R5 (OPEN-214): the bins are outside (`can_start`'s other Home).
        ActionKind::Scavenge => world.leave_building(id),
        // The pre-view pass phase 6 (P15): the order given as the leader reaches the lieutenant.
        ActionKind::Court => crate::systems::leaders::court_start(world, id, target),
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
            // Hunger fix item 11 (b): a grocery run buys the household's
            // shortfall, not the three meals of an errand.
            let units = economy::buy_quantity_upto(
                world,
                id,
                market,
                crate::exec::routine::grocery_cap(world, id).unwrap_or(3),
            );
            // Hunger as sickness § 1: the meal's quality, chosen before the
            // coins move (`Real` with the diet off).
            let q = economy::quality_at(world, id, market);
            let paid = economy::pay_for_food_q(world, id, market, units, q);
            // L2 shadow fixes item 4: the units move with the coins (an
            // interrupted purchase was refunded and bought again: 12 food
            // flows for 6 meals); `(0, 0)` marks a shelf found empty.
            let taken = economy::take_food_q(world, id, market, units, paid, q);
            world.pending_purchase.insert(id, if taken { (units, 0) } else { (0, 0) });
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
        ActionKind::Deal => match target {
            Some(bar) => crate::systems::stims::start_deal(world, id, bar),
            None => crate::systems::stims::start_corner(world, id),
        },
        k if k.is_leisure_step() => crate::systems::leisure::on_start(world, id, k, target),
        // M16a (plan C32): the guard is on post.
        ActionKind::Guard => crate::systems::contracts::on_guard_start(world, id, target),
        // Pressure pass round 7, W3 (`OPEN-244`): the cuffs go on as the guard
        // lays hands on him (the step starts only beside the suspect); the
        // step's minutes are the cuffing. On completion the suspect walked
        // off the tile: 1,143 Arrests in 20k x 30 failed `PreconditionLost`
        // against 1,121 made.
        ActionKind::Arrest => {
            if let Some(s) = target {
                crate::systems::law::arrest(world, id, s);
            }
        }
        // Hunger as sickness § 4: the diver holds the bin for the dive.
        ActionKind::DumpsterDive => {
            let until = world.tick + duration(world, id, kind);
            crate::systems::bins::on_start(world, id, target, until);
        }
        ActionKind::Drink => {
            // M11: the Bar's owner takes the 2 coins (the Treasury owns a city Bar).
            let bar = world.comp::<Position>(id).and_then(|p| p.building);
            let owner = bar.and_then(|b| world.owner_of(b));
            let paid =
                crate::systems::ownership::pay(world, Some(id), owner, 2, crate::systems::ownership::Flow::Drink);
            if let Some(b) = bar {
                crate::systems::ownership::credit(world, b, paid);
                crate::systems::leisure::note_paid(world, id, b, paid);
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
    if crate::systems::fixes::paid_step(kind) {
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
        // Round 7 W5 (`OPEN-237`): a dancer's set cut short is still a set
        // (its tally closed, never carried into the next).
        ActionKind::ClerkWork => crate::systems::vice::set_done(world, id, started),
        // Hunger as sickness § 4: the bin is let go.
        ActionKind::DumpsterDive => {
            let b = world.comp::<Brain>(id).and_then(|b| b.current_step()).and_then(|s| s.target);
            crate::systems::bins::release(world, id, b);
        }
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

/// The perf pass phase 1 (OPEN-195): the departure gate cuts a Rest begun
/// at `started` only if it opened since (with `[rest]`): one begun with the
/// gate open was chosen by a think that saw it and took Idle, and the next
/// 30-tick think still lets Work win. The base cut it every minute while
/// the Work goal did not answer (a rich manager's Work at 0.02 under Idle's
/// 0.05; the departure gate's own refusals are `routine::work_refused`).
fn rest_cut_by_work(world: &World, id: EntityId, started: Tick) -> bool {
    use crate::exec::routine;
    routine::must_leave_for_work(world, id) && !(routine::rest_fix(world) && routine::leave_gate_at(world, id, started))
}

/// The perf pass phase 1 (OPEN-195): `can_work_now` at the post cuts a Rest
/// only on a shift the Work goal would take (with `[rest]`: a super at his
/// own block on the rest day rested in 1-minute blocks all day), and an
/// idle Rest begun on this shift at the post is not cut by it (as the
/// departure gate's, the think that chose it saw the shift). A Work plan's
/// Rest (the commute's wait, the cab's stand-by) is cut as before.
fn rest_cut_on_shift(world: &World, id: EntityId, job: &Job, started: Tick) -> bool {
    use crate::exec::routine;
    if !routine::rest_fix(world) {
        return true;
    }
    if !routine::work_due(world, id, job) {
        return false;
    }
    let work_plan = world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::Work);
    let begun_on_shift = job.on_shift((started % crate::time::TICKS_PER_DAY) as u16)
        && job.shift_key_at(started) == job.shift_key_at(world.tick);
    work_plan || !begun_on_shift
}

/// Some actions end before their nominal duration.
pub fn finishes_early(world: &World, id: EntityId, kind: ActionKind, started: Tick) -> bool {
    match kind {
        // L1: a committed sleep: the commute gate cannot cut the first
        // `sleep_min_ticks` (a long commute made it true most of the day,
        // and the worker slept in one-minute fragments at energy 0).
        ActionKind::Sleep => {
            // L2 shadow fixes item 3: a sleep runs to `sleep_wake_energy`.
            let wake = world.config.life.sleep_wake_energy;
            world.comp::<Needs>(id).is_some_and(|n| n.energy >= wake)
                || ((crate::exec::routine::must_leave_for_work(world, id)
                    // Round 4 T4 (OPEN-217): a Yard's foreman up for the shape-up.
                    || crate::systems::streetwork::foreman_due(world, id))
                    && world.tick.saturating_sub(started) >= world.config.life.sleep_min_ticks)
        }
        ActionKind::Meeting => crate::systems::life::exec_shift_over(world),
        // Lives at work phase 4 (§ 2.3): a cab driver standing by at the
        // Garage rests on until a ride is his (false off: the old rule).
        ActionKind::Rest => {
            rest_cut_by_work(world, id, started)
                || (world
                    .comp::<Job>(id)
                    .is_some_and(|j| can_work_now(world, id, j) && rest_cut_on_shift(world, id, j, started))
                    && !crate::systems::streetwork::standing_by(world, id))
                || crate::systems::streetwork::driving(world, id)
                // Round 3 S5: the rank's wait is up: he cruises to the next.
                || crate::systems::streetwork::rank_wait_over(world, id, started)
        }
        // Waiting at the workplace for the shift: start the moment it begins.
        ActionKind::Wander => world.comp::<Job>(id).is_some_and(|j| can_work_now(world, id, j)),
        // The order changed: the muster breaks up (on_complete then fails the step).
        ActionKind::Muster => crate::systems::raid::raid_done(world, id),
        // Lives at work phase 2 (§ 3.3): nobody came: leave early (false off).
        // Round 7 W2 (OPEN-241): a mustered member waits on his corner for the fight.
        ActionKind::HangOut => {
            crate::systems::turf::muster_wait(world, id).is_none()
                && crate::systems::household::empty_leave(world, id, started)
        }
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
                None => crate::systems::hunt::contact(world, id) && !crate::systems::contracts::locate_waits(world, id),
            }
        }
        _ => false,
    }
}
