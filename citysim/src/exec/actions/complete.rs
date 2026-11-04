use super::*;

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
            let meal = crate::systems::health::eat_inventory_unit(inv);
            inv.stolen_food = inv.stolen_food.min(inv.food);
            if let Some(n) = world.comp_mut::<Needs>(id) {
                needs::eat(n, &cfg);
            }
            crate::systems::health::ate(world, id, meal);
            world.mark_day(id, crate::components::trace_flags::ATE);
            world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            StepResult::Done
        }
        ActionKind::EatAtHome => {
            let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let share = crate::systems::camp::adults_share(world, id, home);
            let Some(b) = world.comp_mut::<Building>(home) else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if b.stock_food == 0 || share == 0 {
                return StepResult::Failed(FailReason::StockGone);
            }
            let meal = crate::systems::health::eat_pantry_unit(b);
            // Round 6 V1 (OPEN-231): off his household's stake first.
            crate::systems::shelf::ate(world, id, home, 1.0);
            world.release_all(id);
            if let Some(n) = world.comp_mut::<Needs>(id) {
                needs::eat(n, &cfg);
            }
            crate::systems::health::ate(world, id, meal);
            world.mark_day(id, crate::components::trace_flags::ATE);
            world.remember(id, MemoryKind::Ate, None, 0.2, 0.3, false);
            StepResult::Done
        }
        ActionKind::StoreFood => {
            let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let cap = world.config.buildings.home.stock_cap;
            // Round 6 V1 (OPEN-231): his household's room, and his stake.
            let room = crate::systems::shelf::room(world, id, home, cap);
            let Some(inv) = world.comp_mut::<Inventory>(id) else { return StepResult::Failed(FailReason::StockGone) };
            let movable = (inv.food - inv.stolen_food).min(room);
            // Hunger as sickness § 1: the synth meals go in with the rest.
            let synth = inv.synth_held().min(movable);
            inv.synth = inv.synth_held() - synth;
            inv.food -= movable;
            inv.synth = inv.synth.min(inv.food);
            if let Some(b) = world.comp_mut::<Building>(home) {
                b.synth_food = b.synth_food.min(b.stock_food) + synth;
                b.stock_food += movable;
            }
            crate::systems::shelf::stored(world, id, movable);
            StepResult::Done
        }
        ActionKind::BuyFood => {
            let (units, _) = world.pending_purchase.remove(&id).unwrap_or((0, 0));
            world.release_all(id);
            // L2 shadow fixes item 4: taken at the start already.
            if units > 0 {
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
                    || here.is_some_and(|b| {
                        world.gang_of(id).is_some_and(|g| crate::systems::vertical::is_hideout_of(world, g, b))
                    }));
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
            // Shadow V4 bug 2: a doze restores up to `[life] rest_energy_cap`,
            // never past it (+0.1 a 90-min Rest outran the decay: a woman
            // looped Rest for four days at 0.85-0.97 and never slept).
            let cap = world.config.life.rest_energy_cap;
            // Lives at work phase 2 (§ 3.5): a Visit's hour at the kin's side.
            if world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::Visit) {
                crate::systems::household::visit_done(world, id);
            }
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.energy = n.energy.max((n.energy + 0.1).min(cap));
            }
            StepResult::Done
        }
        ActionKind::Drink => {
            if let Some(n) = world.comp_mut::<Needs>(id) {
                n.belonging = (n.belonging + 0.15).min(1.0);
            }
            world.remember(id, MemoryKind::Socialised, None, 0.2, 0.3, false);
            // Lives at work phase 1 (§ 2.1): the drink is the bartender's
            // encounter (no coins: a drink never tipped, and tips nobody).
            if let Some(bar) = world.comp::<Position>(id).and_then(|p| p.building) {
                crate::systems::service::arrive(world, id, bar, crate::components::ServeKind::Visit, 0);
            }
            // M15 W7: the rumour mill, one exchange each way with a co-drinker.
            crate::systems::gossip::drink(world, id);
            // L2 L14: a drink's fun; a Club's or Den's visit.
            crate::systems::leisure::drink_fun(world, id);
            crate::systems::leisure::note_drink(world, id);
            // Jobs & lives phase 5 (§ 5.3): a black unit from the Bar's dealer (nothing off).
            if let Some(bar) = world.comp::<crate::components::Position>(id).and_then(|p| p.building) {
                crate::systems::braindance::bar_visit(world, id, bar, true);
            }
            StepResult::Done
        }
        // L2 L14: the leisure steps' completions.
        k if k.is_leisure_step() => {
            crate::systems::leisure::on_complete(world, id, k, target, started);
            StepResult::Done
        }
        // M16a (plan C10, C15): a regular's stamp; a Guard's day kept.
        ActionKind::Network => crate::systems::contracts::on_network(world, id, target),
        // Round 9 Z2: a record off the board taken in person.
        ActionKind::TakeJob => crate::systems::contracts::on_take_job(world, id, target),
        // The Hit market (H9): the want posted at the Fixer.
        ActionKind::Commission => crate::systems::hitmarket::on_commission(world, id, target),
        // M18 P5h: the verdict at the letting desk.
        ActionKind::Apply => crate::systems::housing::on_apply(world, id, target),
        // M16b phase 2: the abductee handed over at the holding building.
        ActionKind::HandOver => crate::systems::captives::on_hand_over(world, id, target),
        // M16b phase 3: the downed taken up; stabilised at the Clinic.
        ActionKind::CarryWounded => crate::systems::wounded::on_carry(world, id, target),
        ActionKind::Stabilise => crate::systems::wounded::on_stabilise(world, id, target),
        // Real economy E27, E28: the meal served, the gift given.
        ActionKind::EatAlms => crate::systems::charity::on_eat_alms(world, id, target),
        ActionKind::DumpsterDive => crate::systems::bins::on_complete(world, id, target),
        // Jobs & lives phase 2: the hour stood (the sales are world-side).
        // Round 4 T5 (OPEN-217): a searcher's ask at a door settled.
        ActionKind::Vend => {
            match target {
                // M18 P2: a directed ask at its bound door.
                Some(b) => crate::systems::search::ask_at(world, id, b),
                None => crate::systems::search::on_ask(world, id),
            }
            StepResult::Done
        }
        // P5i: one pantry unit (synth first, as eaten) into the carried food.
        ActionKind::PackMeal => {
            let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else {
                return StepResult::Failed(FailReason::NoSuchPlace);
            };
            let share = crate::systems::camp::adults_share(world, id, home);
            let Some(b) = world.comp_mut::<Building>(home) else { return StepResult::Failed(FailReason::NoSuchPlace) };
            if b.stock_food == 0 || share == 0 {
                return StepResult::Failed(FailReason::StockGone);
            }
            let meal = crate::systems::health::eat_pantry_unit(b);
            // Round 6 V1 (OPEN-231): off his household's stake first.
            crate::systems::shelf::ate(world, id, home, 1.0);
            world.release_all(id);
            let Some(inv) = world.comp_mut::<Inventory>(id) else { return StepResult::Failed(FailReason::StockGone) };
            let synth = inv.synth_held();
            inv.food += 1;
            inv.synth = synth + u32::from(meal == crate::components::MealKind::Synth);
            StepResult::Done
        }
        ActionKind::Oncology => match target {
            Some(c) if crate::systems::health::oncology(world, id, c) => StepResult::Done,
            _ => StepResult::Failed(FailReason::PreconditionLost),
        },
        ActionKind::Patch => match crate::systems::customers::patch(world, id, target) {
            true => StepResult::Done,
            false => StepResult::Failed(FailReason::PreconditionLost),
        },
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
        ActionKind::Scavenge => {
            // The pressure pass phase 2: a scav's round names its find.
            let before = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
            let r = scavenge(world, id);
            let found = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins) - before;
            crate::systems::customers::scav_note(world, id, target, found);
            r
        }
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
            // The pre-view pass phase 2 (OPEN-137, P5): a stations block
            // before the shift's last closes nothing (the wage and the
            // output wait for the last, at the post).
            if crate::systems::shift::block_done(world, id, started) {
                // Round 3 S4 (OPEN-216): a super's block is a leg (a repair,
                // a complaint, the stairs).
                crate::systems::service::super_leg(world, id, started);
                // Round 4 T4 (OPEN-217): a dancer's block is a set.
                crate::systems::vice::set_done(world, id, started);
                return StepResult::Done;
            }
            // Jobs and room J13: a producing trade's shift makes its output
            // (over the shift's span from its first block, P5).
            if kind == ActionKind::ClerkWork {
                if let Some(b) = world.comp::<Job>(id).filter(|j| j.role.is_trade()).and_then(|j| j.employer) {
                    let from = crate::systems::shift::span_start(world, id, started);
                    crate::systems::trades::accrue(world, id, b, now.saturating_sub(from));
                }
            }
            crate::systems::vice::set_done(world, id, started);
            crate::systems::shift::shift_closed(world, id);
            end_shift(world, id);
            StepResult::Done
        }
        // The pre-view pass phase 2 (P7): back to the stations.
        ActionKind::Break => StepResult::Done,
        // The pre-view pass phase 3: the load set down (a courier's parcels handed over).
        ActionKind::Unload => {
            crate::systems::shift::unloaded(world, id, target);
            StepResult::Done
        }
        // The pre-view pass phase 6 (P14): a court held is counted.
        ActionKind::Court => {
            crate::systems::leaders::court_done(world, id, target);
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
            // The pre-view pass phase 4 (P8): a stop of the round goes on.
            if crate::systems::jobs::sweep_stop_done(world, id, now.saturating_sub(started)) {
                return StepResult::Done;
            }
            crate::systems::jobs::sweep_done(world, id, now.saturating_sub(started));
            end_shift(world, id);
            StepResult::Done
        }
        ActionKind::Wander => StepResult::Done,
        ActionKind::ReportCrime => report_crime(world, id, target),
        ActionKind::PatrolLeg => {
            // OPEN-280: the route loops (`patrol_legs` modulo its length)
            // until the clock ends the shift; five legs used to close it
            // at ~12:30 of a 09:00-18:00 shift, paid in full.
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.patrol_legs = b.patrol_legs.wrapping_add(1);
            }
            let shift_over = world.comp::<Job>(id).is_some_and(|j| !j.on_shift(world.tick_of_day()));
            if shift_over {
                // The last leg runs past the end: `end_shift` reads the shift
                // at `tick - 1`, which for a night guard (21:00-06:00) at
                // 06:15 is tonight's coming shift, marked worked and paid
                // unworked (OPEN-280); the shift closed is the patrol's.
                let key = world.comp::<Brain>(id).and_then(|b| b.patrol_shift_key);
                end_shift(world, id);
                if let (Some(k), Some(j)) = (key, world.comp_mut::<Job>(id)) {
                    j.last_shift_day = Some(k);
                }
                if let Some(b) = world.comp_mut::<Brain>(id) {
                    b.patrol_legs = 0;
                    b.patrol_route.clear();
                }
            }
            StepResult::Done
        }
        ActionKind::Arrest => {
            let Some(suspect) = target else { return StepResult::Failed(FailReason::NoSuchPlace) };
            // Round 7 W3: cuffed at the start (`on_start`); a suspect who
            // fought free there is gone.
            if world.comp::<Brain>(id).is_some_and(|b| b.escorting == Some(suspect)) {
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
            if !(crate::systems::law::near(world, id, partner, 1)
                || crate::goap::actions::street_beside(world, id, partner))
                && world.comp::<Position>(id).and_then(|p| p.building)
                    != world.comp::<Position>(partner).and_then(|p| p.building)
            {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            // Shadow V4 bug 10: no chat ends warmly with someone who became
            // an Enemy mid-chat (the partner robbed the agent, then chatted).
            if world.edge(id, partner).is_some_and(|e| e.kind == crate::components::RelKind::Enemy) {
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
                crate::systems::social::suitor(world, id, partner)
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
                // Round 8 X5 (`OPEN-248`): a date in the pair's courtship.
                crate::systems::social::dated(world, id, partner);
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
            } else if crate::systems::crew::on(world) {
                // R5 (OPEN-214): the try-out decided: turned away, said so.
                crate::systems::gang::turned_away(world, id, target);
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
            let share = crate::systems::gang::split_loot(world, id);
            // The pre-view pass phase 6 (P14): handed to the leader at court.
            crate::systems::leaders::handed(world, id, share);
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
            let food = crate::systems::gang::fence(world, id);
            // Jobs & lives phase 3: the stolen items carried go with the food.
            let items = match world.gang_of(id) {
                Some(g) => crate::systems::weapons::fence_items(world, id, g),
                None => 0,
            };
            if food > 0 || items > 0 {
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
                // Jobs & lives phase 3: a weapon at a Gunsmith or a dealer's Bar.
                (Some(seller), Some(pick)) if target == Some(seller) && pick.kind == AssetKind::Weapon => {
                    match crate::systems::weapons::buy(world, id, seller, &pick, true) {
                        Ok(_) => StepResult::Done,
                        Err(_) => StepResult::Failed(FailReason::PreconditionLost),
                    }
                }
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
            let before = world.purse(Some(id));
            if crate::systems::tech::sell_deck_data(world, id) > 0 {
                // M16a (plan C34): a Fixer's run order takes its cut.
                let sale = world.purse(Some(id)) - before;
                crate::systems::contracts::on_sell_data(world, id, sale);
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
                // Lives at work phase 4b (§ 2.3): on the corner the plan named.
                None if target.is_none() && crate::systems::stims::buy_at_corner(world, id) > 0 => StepResult::Done,
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
            // M15 W35: a guard standing over the body fights for it first
            // (C3a: live, the step holds until it ends).
            match crate::systems::grudges::guard_contests(world, id, c) {
                None => return StepResult::Running,
                Some(false) => return StepResult::Failed(FailReason::PreconditionLost),
                Some(true) => {}
            }
            if !crate::systems::chrome::strip(world, id, c) {
                return StepResult::Failed(FailReason::StockGone);
            }
            // Jobs & lives phase 6 (§ 7): a typed gang's ripper takes the
            // organs of a body with no chrome too (nothing off).
            let organs = crate::systems::gray::wants_organs(world, id, c);
            if crate::systems::chrome::may_rip(world, id)
                && (!crate::systems::chrome::installed(world, c).is_empty() || organs)
            {
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
            // M15 W35: a guard standing over the body fights for it first
            // (C3a: live, the step holds until it ends).
            match crate::systems::grudges::guard_contests(world, id, b) {
                None => return StepResult::Running,
                Some(false) => return StepResult::Failed(FailReason::PreconditionLost),
                Some(true) => {}
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
            // M16b phase 2: a contract's abductor closes in from its stake-out.
            let reach =
                if world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(crate::components::GoalKind::Contract) {
                    4
                } else {
                    1
                };
            // C3a: the fight live (the step holds), then its tail's verdict.
            let done = crate::systems::combat::take_done(world, id, crate::systems::combat::Held::Abduct);
            match done.or_else(|| crate::systems::chrome::abduct_within(world, id, v, reach)) {
                None => StepResult::Running,
                Some(true) => StepResult::Done,
                Some(false) => StepResult::Failed(FailReason::PreconditionLost),
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
        ActionKind::Muster => match crate::systems::raid::depart(world, id) {
            crate::systems::raid::Departure::Go => StepResult::Done,
            // V8b (`OPEN-276`): the muster holds for its column.
            crate::systems::raid::Departure::Hold => StepResult::Running,
            crate::systems::raid::Departure::Off => StepResult::Failed(FailReason::PreconditionLost),
        },
        ActionKind::Brawl => {
            // M12 phase 4: the first at the door waits for the crew.
            if crate::systems::raid::wait_for_crew(world, id) {
                return StepResult::Running;
            }
            // The city milestone C8: the column climbs to the target's floor first.
            if crate::systems::vertical::climb_column(world, id) {
                return StepResult::Running;
            }
            crate::systems::raid::resolve(world, id);
            StepResult::Done
        }
        ActionKind::Attack => {
            let Some(victim) = target else { return StepResult::Failed(FailReason::PartnerLeft) };
            // C3a: the step's engagement ended (its tail ran): done, or the
            // guard beaten and the victim next.
            let mut guarded = false;
            if let Some(go_on) = crate::systems::combat::take_done(world, id, crate::systems::combat::Held::AttackGuard)
            {
                if !go_on {
                    return StepResult::Done;
                }
                guarded = true;
            } else if crate::systems::combat::take_done(world, id, crate::systems::combat::Held::Attack).is_some() {
                return StepResult::Done;
            }
            if !crate::systems::law::living(world, victim) || !crate::systems::law::near(world, id, victim, 4) {
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            // Round 8 X1 (`OPEN-246`): the cells' wall: nobody outside lays a
            // hand on the held, nor the held on anyone outside (V16
            // `street_worker_35520 d22 18:16`: the victim beat the thief held
            // in the Precinct). A cellmate is within reach.
            if crate::systems::law::held_apart(world, id, victim) {
                crate::systems::ownership::jobs2_row(world).held_hit += 1;
                return StepResult::Failed(FailReason::PartnerLeft);
            }
            // M16a (plan C32): a Guard taker on post beside the client meets
            // the attack first; a guard's win ends it (the attacker's Assault).
            if let Some(g) = crate::systems::contracts::guard_for(world, id, victim).filter(|_| !guarded) {
                let tail = crate::systems::combat::Tail::AttackGuard { victim, guard: g };
                match crate::systems::combat::fight(world, id, g, Default::default(), tail) {
                    crate::systems::combat::Begun::Live => return StepResult::Running,
                    crate::systems::combat::Begun::Over(false) => return StepResult::Done,
                    crate::systems::combat::Begun::Over(true) => {}
                }
            }
            // M13 D33: a berserker's blows kill at `berserk_kill_mult`.
            // M15 W22: a lethal Hunt's strike kills at `hunt_kill_p`.
            let mods = crate::systems::law::FightMods {
                kill_mult: crate::systems::chrome::attack_kill_mult(world, id),
                a_bonus: 0.0,
                kill_p: crate::systems::hunt::strike_kill_p(world, id, victim),
                ..Default::default()
            };
            // C3a: live when both are Full (the step holds until it ends);
            // the tail is `attack_end`.
            let tail = crate::systems::combat::Tail::Attack { victim };
            match crate::systems::combat::fight(world, id, victim, mods, tail) {
                crate::systems::combat::Begun::Live => StepResult::Running,
                crate::systems::combat::Begun::Over(_) => StepResult::Done,
            }
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
            } else if crate::systems::hunt::contact(world, id) && !crate::systems::contracts::locate_waits(world, id) {
                // M16a (plan C27): a Locate tracker's contact is a paid sighting.
                crate::systems::contracts::on_contact(world, id);
                // M16a (plan C24, phase 2): a contract Hit or Beat decides its
                // strike at contact (a Hold, a sell-out or a Fail ends here).
                if !crate::systems::contracts::strike_at_contact(world, id) {
                    return StepResult::Failed(FailReason::PreconditionLost);
                }
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
