//! The goal table: requirements and considerations per goal, exactly as in
//! the spec's Goal selection section. Goals whose inputs do not exist yet
//! (crime reports, hostiles, corpses) gate to zero until their milestone.

use crate::components::{
    Brain, BuildingKind, GoalKind, Household, Identity, Inventory, Job, Memory, MemoryKind, Needs, Personality,
    RelKind, Role, Sentence, Skills, Wallet,
};
use crate::entity::EntityId;
use crate::time::{self, DayPhase};
use crate::utility::curves::{can, gate_or, urgency, Curve, GATE, IDENTITY, SQUARE};
use crate::utility::Consideration;
use crate::world::World;

/// Table order, which is also the tie-break order.
pub const GOAL_ORDER: [GoalKind; 32] = [
    GoalKind::Eat,
    GoalKind::Sleep,
    GoalKind::Work,
    GoalKind::Earn,
    // Jobs & lives phase 2: the street's day, right after Earn.
    GoalKind::Pitch,
    GoalKind::Socialise,
    GoalKind::Court,
    GoalKind::Flee,
    GoalKind::Fight,
    // M15 W19: right after Fight.
    GoalKind::Hunt,
    // M16a (plan C13): right after Hunt.
    GoalKind::Contract,
    // M16b phase 3 (spec § 5): right after Contract.
    GoalKind::Rescue,
    // Lives at work phase 2 (§ 3.5): right after Rescue.
    GoalKind::Visit,
    // The Hit market (H9): right after Contract.
    GoalKind::Commission,
    GoalKind::ReportCrime,
    GoalKind::Patrol,
    GoalKind::Arrest,
    GoalKind::JoinGang,
    GoalKind::GangWork,
    GoalKind::Raid,
    // L2 L14: the leader's Collect and Call, after Raid.
    GoalKind::Lead,
    GoalKind::Bury,
    // M15 W35: after Bury.
    GoalKind::GuardBody,
    // M12 D27: before Found.
    GoalKind::Squat,
    // M13 D47: before Found, in the order Shop, GetHigh, Treat, Loot.
    GoalKind::Shop,
    GoalKind::GetHigh,
    GoalKind::Treat,
    GoalKind::Loot,
    // M14 V41: before Found.
    GoalKind::Hack,
    // L2 L14: after Hack, before Found.
    GoalKind::Unwind,
    // M11 D26: just before Idle.
    GoalKind::Found,
    GoalKind::Idle,
];

/// Mood multiplier rules from the Mood table.
fn mood_mult(mood: f32, goal: GoalKind) -> f32 {
    match goal {
        GoalKind::Socialise if mood <= -0.3 => 1.2,
        GoalKind::Socialise | GoalKind::Court if mood > 0.5 => 1.2,
        _ => 1.0,
    }
}

/// Phase gate: 1 in the listed phases, else `lo`.
fn phase_curve(current: DayPhase, phases: &[DayPhase], lo: f32) -> Consideration {
    let x = can(phases.contains(&current));
    Consideration::new("phase", x, gate_or(lo))
}

/// The spec's `in shift now` gate, read so that the walk to work and the
/// wait at the door are part of the shift: see `routine::work_pending`.
pub fn shift_reachable(world: &World, id: EntityId, job: &Job) -> bool {
    crate::exec::routine::work_pending(world, id, job)
}

/// Had today's drink (the one-per-day rule).
pub fn drank_today(world: &World, id: EntityId) -> bool {
    world
        .comp::<Memory>(id)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Socialised && time::day(e.tick) == world.day()))
}

/// The agent's Bar (the one it is in, else the nearest) is at capacity and
/// this agent is not inside it.
pub fn bar_full_for(world: &World, id: EntityId) -> bool {
    world
        .local(id, BuildingKind::Bar)
        .and_then(|b| world.comp::<crate::components::Building>(b))
        .is_some_and(|b| b.is_full() && !b.occupants.contains(&id))
}

/// Is the goal's GOAP goal state already true? Such a goal has nothing to
/// plan and is skipped before scoring, like one missing a component.
/// Otherwise Eat beats Idle at hunger 0.8 and most of every meal is wasted.
pub fn already_satisfied(world: &World, id: EntityId, goal: GoalKind, has_spouse: bool) -> bool {
    use crate::goap::world_state::{BELONGING_SATISFIED, ENERGY_SATISFIED, HUNGER_SATISFIED, SAFE, SAVINGS_DAYS};
    let needs = world.comp::<Needs>(id);
    match goal {
        // Hunger fix item 11 (b): a fed worker's grocery run is Eat's too.
        GoalKind::Eat => {
            needs.is_some_and(|n| n.hunger >= HUNGER_SATISFIED) && !crate::exec::routine::grocery_due(world, id)
        }
        GoalKind::Sleep => needs.is_some_and(|n| n.energy >= ENERGY_SATISFIED),
        GoalKind::Socialise => {
            // Until Chat arrives (M5) a drink is the only satisfier: the goal is
            // off for the day once had, and off while it cannot be had (no 2
            // coins, or the Bar is visibly full). Otherwise it wins every think
            // and churns through plan failures.
            // Satisfied, or nothing to do for it: no Chat partner here and no drink possible.
            let chat_possible = chat_venue(world, id)
                && crate::systems::social::best_colocated_partner(world, id, -1.0, false).is_some();
            // L2 L16: a HangOut spot in reach is something to do.
            needs.is_some_and(|n| n.belonging >= BELONGING_SATISFIED)
                || (!chat_possible
                    && (world.comp::<Wallet>(id).is_some_and(|w| w.coins < 2)
                        || bar_full_for(world, id)
                        || drank_today(world, id)
                        // The island pass phase 2: the Bar past the spot cap.
                        || crate::systems::leisure::bar_past_cap(world, id))
                    && !crate::systems::leisure::spot_in_reach(world, id))
        }
        GoalKind::Flee => needs.is_some_and(|n| n.safety >= SAFE),
        GoalKind::Earn => {
            // Shadow V4 bug 5: an exec draws a salary and steals no vehicle
            // (a 10 c day read as poverty and fell through to StealVehicle).
            if !world.has::<Job>(id) && crate::systems::life::exec_corp(world, id).is_some() {
                return true;
            }
            let price = world.local(id, BuildingKind::Market).map_or(i64::MAX, |m| world.price_for(m, id));
            // Hunger as sickness § 3: a diagnosed patient saves for the treatment too.
            let line = SAVINGS_DAYS.saturating_mul(price).max(crate::systems::health::treatment_savings(world, id));
            let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
            if world.has::<Wallet>(id) && coins >= line {
                return true;
            }
            // OPEN-138: the planner's own savings line (`WorldState::
            // has_savings`, the price the agent pays at that Market,
            // `diet::price_at`) holds too: Earn has nothing to plan, so it
            // is no candidate (it won over Eat and planned empty, the idle
            // freeze; `plan::install` cools any empty plan as the net).
            let paid = world
                .local(id, BuildingKind::Market)
                .map_or(1, |m| crate::systems::diet::price_at(world, id, m))
                .max(1);
            if world.has::<Wallet>(id)
                && coins >= SAVINGS_DAYS.saturating_mul(paid)
                && coins >= crate::systems::health::treatment_savings(world, id)
            {
                return true;
            }
            // Nothing to plan: an unemployed agent whose dole is taken or
            // suspended (Beg yields coins, never savings), or an employed one
            // off shift with no wage due (the in-shift case stays scored, as in
            // the spec's worked example; it loses to Work's hysteresis).
            let nothing = match world.comp::<Job>(id) {
                // L1: wages are paid at the shift's end; off shift there is
                // nothing to earn unless the agent cannot buy a meal.
                Some(j) => !j.on_shift(world.tick_of_day()) && !crate::systems::life::broke(world, id),
                // L1: an exec draws a salary; the rest wait for the dole to
                // accrue (one Hall visit pays up to a week), and the broke
                // (with no dole, the short: Shadow V4 bug 1) keep Earn alive
                // (Beg, Scavenge).
                None => {
                    crate::systems::life::exec_corp(world, id).is_some()
                        || (!crate::systems::life::earn_short(world, id)
                            && (!crate::systems::life::dole_trip_due(world, id)
                                || world.treasury().is_some_and(|t| t.coins < 0)
                                || world.levers.dole_per_day == 0))
                }
            };
            // M13 D26: a vehicle within reach to steal, and a gang at the
            // Hideout to pay for it, is something to plan; so (review fix) is
            // the fence of a stolen vehicle already held.
            nothing
                && !crate::systems::vehicles::theft_plannable(world, id)
                && !crate::systems::vehicles::fence_plannable(world, id)
        }
        GoalKind::Court => has_spouse,
        GoalKind::GangWork => {
            !world.has::<crate::components::GangMember>(id)
                || (world.comp::<Brain>(id).is_some_and(|b| b.gang_task_day == Some(world.day()))
                    // Lives at work phase 4b (§ 2.3): a pimp's round runs every pitch hour.
                    && crate::systems::vice::pimp_target(world, id).is_none())
        }
        // A guard never scores JoinGang (`considerations` says None): skip the scan.
        // Pressure pass round 2, R5 (OPEN-214): with `[crew]`, and a try-out place in reach.
        GoalKind::JoinGang => {
            world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
                || if crate::systems::crew::on(world) {
                    !crate::systems::gang::tryout_open(world, id)
                } else {
                    !crate::systems::gang::eligible(world, id)
                }
        }
        // M12 D31: a rioter has an expedition too (raid_done is true without one).
        GoalKind::Raid => crate::systems::raid::raid_done(world, id),
        GoalKind::Work => world.comp::<Job>(id).is_some_and(|j| {
            j.last_shift_day == Some(j.next_shift_key(world.tick)) && !crate::exec::routine::wage_pending(world, j)
        }),
        // M11 D26: nothing to found (also the eligibility gate).
        // The pressure pass phase 4 (OPEN-194): or the desk past the reach
        // (F1: the nearest of the Hall and the Stations, `founding::registry`).
        GoalKind::Found => {
            !crate::systems::founding::can_found(world, id)
                || crate::systems::founding::registry(world, id)
                    .is_some_and(|h| crate::systems::distance::out_of_reach(world, id, h, |r| r.register_ticks))
        }
        // M12 D27: squatting already, or nowhere to squat (the eligibility gate).
        GoalKind::Squat => !crate::systems::street::can_squat(world, id),
        // M13 D43: nothing affordable, on cooldown, in arrears or no open seller.
        GoalKind::Shop => crate::systems::assets::shop_choice(world, id, true).is_none(),
        // M13 D34: nothing to treat at `treat_below`; D39: and no addiction
        // within 0.2 of `hooked` to Detox.
        GoalKind::Treat => {
            // The pressure pass phase 2: a wound or a short cancer's care trip.
            let care = crate::systems::customers::care_choice(world, id).is_some();
            (!care
                && !crate::systems::chrome::wants_treatment(world, id)
                && !crate::systems::stims::wants_detox(world, id)
                && !crate::systems::health::wants_oncology(world, id))
                // The pressure pass phase 4 (OPEN-194): the nearest Clinic
                // past the reach (V8: a madam's 208-278 min, five mornings).
                // A diagnosed cancer is not a discretionary trip: walked to
                // at any distance. Pressure pass R3: nor is a care trip
                // (`care_choice` reads its own reach: the back room's is
                // longer; the gate cut every back-room patient past the
                // nearest Clinic's 120 minutes).
                || (!crate::systems::health::wants_oncology(world, id)
                    && !(care && crate::systems::care::cfg(world).is_some())
                    && clinic_out_of_reach(world, id))
        }
        // M13 D39: high already, or no dose in hand and none to buy.
        GoalKind::GetHigh => !crate::systems::stims::wants_high(world, id),
        // M13 D35: no body within reach to strip.
        GoalKind::Loot => crate::systems::chrome::loot_target(world, id).is_none(),
        // M14 V29: no deck, chair or target worth the risk.
        GoalKind::Hack => crate::systems::virt::hack_choice(world, id).is_none(),
        // L2 L14: fun at `fun_satisfied` (or leisure off); the leader's
        // Collect or Call not due.
        GoalKind::Unwind => crate::systems::leisure::unwind_satisfied(world, id),
        GoalKind::Lead => crate::systems::leisure::lead_satisfied(world, id),
        _ => false,
    }
}

/// The pressure pass phase 4 (OPEN-194): the nearest Clinic (open or not)
/// is past `[distance.reach] clinic_ticks` from where `id` sets out (none
/// at all: past). `false` off.
fn clinic_out_of_reach(world: &World, id: EntityId) -> bool {
    if crate::systems::distance::reach(world).is_none() {
        return false;
    }
    let Some(tile) = crate::systems::life::origin(world, id) else { return false };
    crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, tile, false)
        .is_none_or(|c| crate::systems::distance::out_of_reach(world, id, c, |r| r.clinic_ticks))
}

/// Eat's "can afford/obtain" gate: food in hand, a meal's coins, a pantry,
/// lawless enough to steal, or (hunger fix G2) a stocked Mission in reach
/// that would serve this agent. Only Eat reads the local price: one Market
/// scan per think, not per goal.
pub fn eat_obtainable(world: &World, id: EntityId) -> bool {
    let food = world.comp::<Inventory>(id).is_some_and(|i| i.food > 0);
    let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let pantry = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<crate::components::Building>(h))
        .map_or(0, |b| b.stock_food);
    let lawfulness = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    let price =
        world.local(id, BuildingKind::Market).map_or(i64::MAX, |m| crate::systems::diet::price_at(world, id, m));
    food || coins >= price
        || pantry > 0
        || lawfulness < 0.3
        || (crate::systems::fixes::mission_gate(world) && crate::systems::charity::alms_target(world, id).is_some())
        // Hunger as sickness § 4: a stocked bin in reach.
        || crate::systems::bins::dive_target(world, id).is_some()
}

/// Considerations and flat bonus for one goal, or `None` if the agent lacks a
/// required component.
pub fn considerations(
    world: &World,
    id: EntityId,
    goal: GoalKind,
    has_spouse: bool,
) -> Option<(Vec<Consideration>, f32)> {
    considerations_with(world, id, goal, has_spouse, None)
}

/// [`considerations`] with the Court gate's daily pre-filter given
/// (`crate::utility::think_scored`); `None` reads the brain's
/// `court_candidate`.
pub fn considerations_with(
    world: &World,
    id: EntityId,
    goal: GoalKind,
    has_spouse: bool,
    court: Option<bool>,
) -> Option<(Vec<Consideration>, f32)> {
    let needs = world.comp::<Needs>(id);
    let pers = world.comp::<Personality>(id);
    let mood = world.comp::<crate::components::Mood>(id).map_or(0.0, |m| m.value);
    let phase = world.phase();
    let tod = world.tick_of_day();
    let mm = mood_mult(mood, goal);

    let mut flat = 0.0;
    // Hunger fix G1: starving with a meal in reach, the optional and paid
    // goals wait for Eat (the diaries' starving beggar spent the begged
    // coins on a Drink; an exhausted one slept to hunger 0 with 60 coins).
    if matches!(
        goal,
        GoalKind::Socialise
            | GoalKind::Court
            | GoalKind::Unwind
            | GoalKind::Shop
            | GoalKind::GetHigh
            | GoalKind::Hack
            | GoalKind::Found
            | GoalKind::Squat
    ) && crate::systems::fixes::starving_first(world, id)
    {
        return None;
    }
    let eat_first = goal == GoalKind::Sleep && crate::systems::fixes::eat_first(world, id);
    // Hunger fix G2: a free Mission meal before an hour's scavenging.
    if goal == GoalKind::Earn && crate::systems::fixes::free_meal_first(world, id) {
        return None;
    }
    let cs = match goal {
        GoalKind::Eat => {
            let n = needs?;
            world.comp::<Inventory>(id)?;
            world.comp::<Wallet>(id)?;
            // Hunger fix item 11 (b): fed, the goal is the grocery run, at
            // `[life] grocery_score` (an errand: below a real need).
            if n.hunger >= crate::goap::world_state::HUNGER_SATISFIED {
                let mut s = world.config.life.grocery_score.clamp(0.01, 1.0);
                // Hunger fix item 12 (P5j): past a work night's bedtime.
                if crate::exec::routine::past_bedtime(world, id) {
                    s *= world.config.life.work_night_unwind_mult.clamp(0.01, 1.0);
                }
                return Some((vec![Consideration::raw("pantry under a shift's meals", s, s)], 0.0));
            }
            let can_eat = eat_obtainable(world, id);
            let mut cs = vec![
                Consideration::new("U(hunger)", urgency(n.hunger), SQUARE),
                Consideration::new("can afford/obtain", can(can_eat), GATE),
            ];
            // L2 shadow fixes item 3: the tired sleep before they shop
            // (Eat cut the Friday worker's sleep at 06:15 three days
            // running), unless the hunger is real.
            if n.hunger >= 0.25 {
                if let Some(c) = energy_brake(world, n) {
                    cs.push(c);
                }
            }
            cs
        }
        GoalKind::Sleep => {
            let n = needs?;
            let housed = world.comp::<Household>(id).is_some_and(|h| h.home.is_some());
            let lo = match phase {
                DayPhase::Night => 1.0,
                DayPhase::Evening => 0.7,
                // L1: the housed sleep at night (sleep came in 4 h chunks at any hour).
                _ if housed => world.config.life.housed_day_sleep,
                _ => 0.4,
            };
            // L1: exhaustion wins (agents sat at energy 0 for a day), and
            // the night belongs to sleep for anyone off shift.
            // Hunger fix G1: starving with a meal in reach, the flats wait
            // for the meal (Sleep keeps its need and phase terms).
            if n.energy < world.config.life.exhausted_energy && !eat_first {
                flat += world.config.life.exhausted_flat;
            }
            if phase == DayPhase::Night && !world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod)) && !eat_first {
                flat += world.config.life.night_sleep_flat;
            }
            // Hunger fix item 12 (P5j): a work night's bedtime presses.
            if !eat_first {
                flat += world.config.life.work_night_sleep_flat * crate::exec::routine::work_night_pressure(world, id);
            }
            vec![
                Consideration::new("U(energy)", urgency(n.energy), Curve::Logistic { k: 10.0, mid: 0.75 }),
                Consideration::raw("phase", can(phase == DayPhase::Night), lo),
            ]
        }
        GoalKind::Work => {
            let n = needs?;
            let Some(job) = world.comp::<Job>(id) else {
                // L1: an exec's office hours at the corp's HQ.
                crate::systems::life::exec_day_pending(world, id)?;
                let p = pers?;
                let cs = vec![
                    Consideration::new("office hours", 1.0, GATE),
                    Consideration::new("energy", n.energy, Curve::Linear { m: 0.6, b: 0.4 }),
                    Consideration::new("greed", p.greed, Curve::Linear { m: 0.3, b: 0.7 }),
                ];
                return Some((cs, 0.0));
            };
            if world.has::<Sentence>(id) {
                return None;
            }
            let mut cs = vec![
                // Hunger as sickness § 2: a sick day is no shift. The perf
                // pass phase 1: the refusals shared with the departure gate
                // (a sentence returned above; a week unpaid zeroes `paid`
                // too, so the goal is unavailable either way).
                Consideration::new(
                    "in shift",
                    can(shift_reachable(world, id, job) && !crate::exec::routine::work_refused(world, id, job)),
                    GATE,
                ),
                Consideration::new("energy", n.energy, IDENTITY),
                // Wages owed press too (M10 phase 5c): a guard collects only
                // by winning Work after a Patrol shift, and a guard with coins
                // in hand never did, so the week ran out and they quit unpaid.
                Consideration::new(
                    "U(wealth) or wages owed",
                    urgency(n.wealth).max((f32::from(job.days_unpaid) / 4.0).min(1.0)),
                    Curve::Logistic { k: 8.0, mid: 0.5 },
                ),
                Consideration::new("paid", can(job.days_unpaid < 7), GATE),
            ];
            // Shadow V4 bug 19: a worker hungry past `shift_hunger_break`
            // with a meal in reach takes a meal break (only then: a term at
            // 1 would move every Work score through the compensation; review
            // 6: with no meal to break for, as `fixes::emergency`, the
            // shift holds at full weight).
            if n.hunger < world.config.life.shift_hunger_break && eat_obtainable(world, id) {
                cs.push(Consideration::raw("fed", n.hunger, 0.3));
            }
            cs
        }
        GoalKind::Earn => {
            let n = needs?;
            world.comp::<Wallet>(id)?;
            world.comp::<Skills>(id)?;
            let p = pers?;
            // The pressure pass phase 1 (OPEN-178): a skipped shift's hours are his own.
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod))
                && !crate::systems::misery::skipped_now(world, id);
            let mut cs = vec![
                Consideration::new("U(wealth)", urgency(n.wealth), SQUARE),
                Consideration::new("U(hunger)", urgency(n.hunger), IDENTITY),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.3)),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.5, b: 0.5 }),
            ];
            // L2 shadow fixes item 3 (the worker slept in 25-min pieces while
            // Earn held 0.899).
            if let Some(c) = energy_brake(world, n) {
                cs.push(c);
            }
            cs
        }
        GoalKind::Socialise => {
            let n = needs?;
            let p = pers?;
            let mood_x = (mood + 1.0) / 2.0;
            let mut cs = vec![
                Consideration::new("U(belonging)", urgency(n.belonging), Curve::Logistic { k: 6.0, mid: 0.5 }),
                Consideration::raw("mood", mood_x, IDENTITY.eval(mood_x) * mm),
                Consideration::new("sociability", p.sociability, IDENTITY),
                // Day gaps fix round: a job holder's evening is its own (off
                // hours outside the small hours: a night crew's afternoon, an
                // evening shift's morning, a rest day), not the wall clock's
                // (fighter_1065 scored Socialise 0.089 at 08:31 off shift, under
                // Idle). The jobless and execs keep the Evening.
                match world.comp::<Job>(id) {
                    Some(_) => Consideration::new(
                        "phase",
                        can(phase != DayPhase::Night && crate::systems::leisure::off_hours(world, id)),
                        gate_or(0.4),
                    ),
                    None => phase_curve(phase, &[DayPhase::Evening], 0.4),
                },
            ];
            // L1: a drink two hours' walk off is not worth the walk; company
            // here is free.
            let here = chat_venue(world, id)
                && crate::systems::social::best_colocated_partner(world, id, -1.0, false).is_some();
            let tiles = if here {
                0
            } else {
                world
                    .local(id, BuildingKind::Bar)
                    .and_then(|b| crate::systems::life::tiles_to(world, id, b))
                    .unwrap_or(0)
            };
            cs.push(crate::systems::life::travel(world, tiles));
            cs
        }
        GoalKind::Court => {
            let n = needs?;
            let p = pers?;
            let ident = world.comp::<Identity>(id)?;
            if ident.age_days < 18 * time::DAYS_PER_YEAR as u32 || has_spouse {
                return None;
            }
            // Daily pre-filter (any edge >= 0.3 to an unmarried agent), then a
            // target the plan can act on now: someone here, or at a venue.
            let candidate = court
                .unwrap_or_else(|| world.comp::<Brain>(id).and_then(|b| b.court_candidate).is_some_and(|(_, c)| c))
                && crate::systems::social::court_target(world, id).is_some();
            // A widowed spouse grieves for 14 days (Social); a Courted memory toward
            // the same subject adds +0.1 (Memory readers).
            let widowed =
                ident.spouse_died_tick.is_some_and(|t| world.tick.saturating_sub(t) < 14 * time::TICKS_PER_DAY);
            let grief = widowed
                || world.comp::<Memory>(id).is_some_and(|m| {
                    m.entries.iter().any(|e| {
                        e.kind == MemoryKind::Grief && world.tick.saturating_sub(e.tick) < 14 * time::TICKS_PER_DAY
                    })
                });
            if grief {
                return None;
            }
            let intimacy = Consideration::new("U(intimacy)", urgency(n.intimacy), SQUARE);
            vec![
                Consideration::raw("U(intimacy)", intimacy.input, intimacy.output * mm),
                Consideration::new("candidate", can(candidate), GATE),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.6, b: 0.4 }),
                phase_curve(phase, &[DayPhase::Evening], 0.5),
            ]
        }
        GoalKind::Flee => {
            let n = needs?;
            let p = pers?;
            // Memory reader: Fought or Lost in the last two days with the other
            // party within 8 tiles is +0.15.
            let two_days = world.tick.saturating_sub(2 * time::TICKS_PER_DAY);
            let shaken = world.comp::<Memory>(id).is_some_and(|m| {
                m.entries.iter().any(|e| {
                    matches!(e.kind, MemoryKind::Fought | MemoryKind::Lost)
                        && e.tick >= two_days
                        && e.subject.is_some_and(|s| crate::systems::law::near(world, id, s, 8))
                })
            });
            if shaken {
                flat += 0.15;
            }
            // M14 V12: a dazed runner cannot flee the chair.
            if world.comp::<Brain>(id).and_then(|b| b.dazed_until).is_some_and(|t| t > world.tick) {
                return None;
            }
            vec![
                Consideration::new("U(safety)", urgency(n.safety), Curve::Logistic { k: 12.0, mid: 0.5 }),
                // A guard is hostile to the wanted; so is anyone on an Enemy edge.
                Consideration::new(
                    "hostile near",
                    can((crate::systems::law::wanted(world, id)
                        && crate::systems::law::guard_within(world, id, world.config.crime.sight))
                        || hostile_near(world, id, world.config.crime.sight)),
                    GATE,
                ),
                Consideration::new("1-courage", 1.0 - p.courage, Curve::Linear { m: 0.8, b: 0.2 }),
            ]
        }
        GoalKind::Fight => {
            // M13 D33: an episode pins Fight: 1.0 flat, nothing else counts.
            if crate::systems::chrome::in_episode(world, id) {
                return Some((vec![Consideration::new("episode", 1.0, IDENTITY)], 1.0));
            }
            let n = needs?;
            let s = world.comp::<Skills>(id)?;
            let p = pers?;
            if mood < -0.6 {
                flat = 0.15;
            }
            // M13 D33: an edgy body is spoiling for it (a branch: 0 is M12).
            let edgy = crate::systems::chrome::fight_flat(world, id);
            if edgy != 0.0 {
                flat += edgy;
            }
            // Memory readers: a Fought or Lost memory in the last two days means
            // the matter rests (a lost fight ends it; a fought enemy is not fought
            // again at once); WasRobbed by an enemy in reach is +0.2.
            let two_days = world.tick.saturating_sub(2 * time::TICKS_PER_DAY);
            let mem = world.comp::<Memory>(id);
            let lost_recently =
                mem.is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Lost && e.tick >= two_days));
            if lost_recently {
                return None;
            }
            // Shadow V4 bug 3: nobody wakes from their bed to settle a grudge.
            let asleep = world.comp::<Brain>(id).is_some_and(|b| {
                matches!(b.exec, crate::exec::ExecState::Use { kind: crate::goap::ActionKind::Sleep, .. })
            });
            if asleep {
                return None;
            }
            let target = fight_target(world, id);
            let fresh_enemy = target.is_some();
            let robbed_by_enemy = mem.is_some_and(|m| {
                m.entries.iter().any(|e| {
                    e.kind == MemoryKind::WasRobbed
                        && e.subject.is_some_and(|s| {
                            world.enemies_of(id).any(|o| o == s) && crate::systems::law::near(world, id, s, 4)
                        })
                })
            });
            // Shadow V4 bug 3: the robbed's bonus by courage (a flat +0.2 sent
            // the timid after a 1 c robber over ReportCrime).
            if robbed_by_enemy {
                flat += 0.2 * p.courage;
            }
            // M15 W26: a refused shakedown's backlash, while the actor is near.
            if let Some((actor, bonus, _)) = world.comp::<crate::components::Brain>(id).and_then(|b| b.fight_bonus) {
                if crate::systems::moves::fight_bonus_on(world, id, actor).is_some()
                    && crate::systems::law::near(world, id, actor, 4)
                {
                    flat += bonus;
                }
            }
            // Shadow V4 bug 3: the law and the odds weigh in (a lawful, timid
            // reporter assaulted a 1 c robber before three neighbours; old
            // workers picked fights with the young).
            let lc = &world.config.life;
            let odds = target.map_or(0.5, |t| fight_odds(world, id, t));
            vec![
                Consideration::new("enemy near", can(fresh_enemy), GATE),
                Consideration::new("courage", p.courage, IDENTITY),
                Consideration::new("fighting", s.fighting, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("U(safety)", urgency(n.safety), Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("lawless", p.lawfulness, Curve::Linear { m: -lc.fight_lawful_damp, b: 1.0 }),
                Consideration::new("odds", odds, Curve::Linear { m: 1.0, b: lc.fight_odds_floor }),
            ]
        }
        GoalKind::ReportCrime => {
            let p = pers?;
            let mem = world.comp::<Memory>(id)?;
            let two_days = world.tick.saturating_sub(2 * time::TICKS_PER_DAY);
            let saw = mem
                .entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && e.tick >= two_days && !e.second_hand)
                .filter(|e| e.subject.is_some_and(|s| !crate::systems::law::reported_since(world, s, e.tick)))
                // The pressure pass phase 5a: a crime kept to sell is not reported.
                .filter(|e| e.subject.is_none_or(|s| !crate::systems::consequence::keeps_quiet(world, id, s)))
                .max_by(|a, b| a.salience.partial_cmp(&b.salience).unwrap_or(std::cmp::Ordering::Equal))?;
            let in_gang = world.has::<crate::components::GangMember>(id);
            let betraying = world.comp::<Brain>(id).is_some_and(|b| b.betraying);
            if in_gang && !betraying {
                return None;
            }
            let hall_open = matches!(phase, DayPhase::Morning | DayPhase::Work | DayPhase::Evening);
            let affinity = saw
                .subject
                .and_then(|s| world.edges.get(&crate::components::edge_key(id, s)))
                .map_or(0.0, |e| e.affinity);
            // A betraying gang member gets +0.3 (Gang › Betrayal).
            if betraying {
                flat = 0.3;
            }
            let mut cs = vec![
                Consideration::new("lawfulness", p.lawfulness, Curve::Logistic { k: 8.0, mid: 0.5 }),
                Consideration::new("salience", saw.salience, IDENTITY),
                Consideration::new("hall open", can(hall_open), GATE),
                Consideration::new("1-affinity(suspect)", (1.0 - affinity).clamp(0.0, 1.0), IDENTITY),
            ];
            // L1: the walk to the Hall weighs against a report.
            let hall = world.building_of_kind(BuildingKind::Hall);
            let tiles = hall.and_then(|h| crate::systems::life::tiles_to(world, id, h)).unwrap_or(0);
            cs.push(crate::systems::life::travel(world, tiles));
            // The pressure pass phase 4 (OPEN-194): a Hall past the reach is
            // not walked to for a report (V8: 577 min and a shift lost).
            if crate::systems::distance::reach(world).is_some() {
                let near =
                    hall.is_none_or(|h| !crate::systems::distance::out_of_reach(world, id, h, |r| r.report_ticks));
                cs.push(Consideration::new("hall within reach", can(near), GATE));
            }
            cs
        }
        GoalKind::Patrol => {
            let job = world.comp::<Job>(id)?;
            if job.role != Role::Guard || world.has::<Sentence>(id) {
                return None;
            }
            let n = needs?;
            let key = job.next_shift_key(world.tick);
            let patrol_day = !crate::systems::law::jail_duty(world, id, key);
            let legs_left =
                world.comp::<Brain>(id).is_some_and(|b| b.patrol_legs < world.config.crime.patrol_legs_per_shift);
            let mut on_duty = job.on_shift(tod) && patrol_day && job.last_shift_day != Some(key) && legs_left;
            // L2 shadow fixes item 10: no patrol on the rest day (unpaid:
            // `law::credit_guard_shifts` reads the workday).
            on_duty &= crate::exec::routine::workday_of(world, id, job, key);
            // The same reach as Arrest's gate (M10 phase 5c review): a
            // located warrant across the city scaled every guard's Patrol by
            // 0.3 while only the guards in range could Arrest, so the rest
            // idled, never walked a leg and were never owed the shift.
            let here = world.comp::<crate::components::Position>(id).map(|p| p.tile).unwrap_or_default();
            let by = crate::systems::law::Pursuer { tile: here, guard: id };
            let mut no_warrant = !crate::systems::law::any_located_suspect(world, Some(by));
            // L1: a chase under way is a warrant (see Arrest).
            if no_warrant {
                no_warrant = !world
                    .comp::<Brain>(id)
                    .and_then(|b| b.plan.as_ref())
                    .filter(|p| p.goal == GoalKind::Arrest)
                    .and_then(|p| p.target)
                    .is_some_and(|s| crate::systems::law::is_located_suspect(world, s));
            }
            vec![
                Consideration::new("in shift", can(on_duty), GATE),
                Consideration::new("energy", n.energy, Curve::Linear { m: 0.8, b: 0.2 }),
                Consideration::new("no warrant", can(no_warrant), gate_or(0.3)),
            ]
        }
        GoalKind::Arrest => {
            let job = world.comp::<Job>(id)?;
            if job.role != Role::Guard {
                return None;
            }
            let p = pers?;
            // M10: only warrants within pursuit range of this guard.
            let here = world.comp::<crate::components::Position>(id).map(|p| p.tile).unwrap_or_default();
            let by = crate::systems::law::Pursuer { tile: here, guard: id };
            let escorting = world.comp::<Brain>(id).is_some_and(|b| b.escorting.is_some());
            // L1: on shift only, tired guards less, the nearest warrant
            // weighed by its walk (unclaimed: one guard per suspect).
            let n = needs?;
            // The suspect this guard is already after stays its warrant
            // while located (the sighting aged past the fresh window
            // mid-chase and Patrol took the guard off it).
            let chasing = world
                .comp::<Brain>(id)
                .and_then(|b| b.plan.as_ref())
                .filter(|p| p.goal == GoalKind::Arrest)
                .and_then(|p| p.target)
                .filter(|&s| crate::systems::law::is_located_suspect(world, s));
            let dist = |s: EntityId| world.last_seen().get(&s).map(|&(t, _)| t.manhattan(here));
            // Pressure pass round 3, S3 (OPEN-223): the suspect this guard
            // saw do it himself is his collar, on shift or off.
            let collar = if escorting { None } else { crate::systems::law::collar_of(world, id) };
            let nearest = if escorting {
                Some(0)
            } else if let Some(s) = chasing.or(collar) {
                dist(s)
            } else {
                // Shadow V4 bug 11: a corp guard's warrants on its beat only
                // (`law::located_suspects` reads `on_beat`).
                crate::systems::law::located_suspects(world, Some(by)).into_iter().filter_map(dist).min()
            };
            // L2 shadow fixes item 10: the shift is a workday's.
            let working =
                job.on_shift(tod) && crate::exec::routine::workday_of(world, id, job, job.shift_key_at(world.tick));
            let on = if working || escorting || collar.is_some() { 1.0 } else { world.config.life.arrest_off_shift };
            return Some((
                vec![
                    Consideration::new("warrant located", can(nearest.is_some()), GATE),
                    Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                    Consideration::raw("in shift", on, on),
                    Consideration::new("energy", n.energy, Curve::Linear { m: 0.8, b: 0.2 }),
                    crate::systems::life::travel(world, nearest.unwrap_or(0)),
                ],
                // S3: his collar outweighs the commute or the shift's desk.
                if collar.is_some() { world.config.justice.as_ref().map_or(0.0, |c| c.collar_flat) } else { 0.0 },
            ));
        }
        GoalKind::JoinGang => {
            if world.has::<crate::components::GangMember>(id)
                || world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
            {
                return None;
            }
            let n = needs?;
            let p = pers?;
            // The pressure pass phase 1 (OPEN-178): the miserable are easy recruits.
            flat += crate::systems::misery::join_flat(world, id);
            vec![
                Consideration::new("1-lawfulness", 1.0 - p.lawfulness, SQUARE),
                Consideration::new("U(wealth)", urgency(n.wealth), IDENTITY),
                // `already_satisfied` has just checked eligibility (it skips
                // JoinGang for the ineligible), so the gate is open here.
                Consideration::new("eligible", can(true), GATE),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
            ]
        }
        GoalKind::GangWork => {
            world.comp::<crate::components::GangMember>(id)?;
            let n = needs?;
            let p = pers?;
            // Lives at work phase 4b (§ 2.3): a pimp's round in pitch hours
            // is his shift (scored as the exec's office hours).
            if crate::systems::vice::pimp_target(world, id).is_some() {
                let t = &world.config.service.tune;
                let cs = vec![
                    Consideration::new("pitch hours", 1.0, GATE),
                    Consideration::new("energy", n.energy, Curve::Linear { m: t.pimp_energy.0, b: t.pimp_energy.1 }),
                    Consideration::new(
                        "loyalty",
                        p.loyalty,
                        Curve::Linear { m: t.pimp_loyalty.0, b: t.pimp_loyalty.1 },
                    ),
                ];
                return Some((cs, 0.0));
            }
            // The pressure pass phase 2 (OPEN-176): a scav's off hours are
            // the gang's round of the bins (a Harvest target outranks it).
            if crate::systems::gang::gang_work_target(world, id).is_none()
                && crate::systems::customers::scav_bin(world, id).is_some()
            {
                let cs = vec![
                    Consideration::new("the scav round", 1.0, GATE),
                    Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.5, b: 0.3 }),
                    Consideration::new("energy", n.energy, Curve::Logistic { k: 10.0, mid: 0.3 }),
                    Consideration::new("loyalty", p.loyalty, Curve::Linear { m: 0.4, b: 0.4 }),
                ];
                return Some((cs, 0.0));
            }
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            // The order tilts the day: a loyal member following orders scores
            // 1.0 here, a freelancer 1 - order_weight. Hunger and fear still win.
            // `already_satisfied` has just scanned for a target (and skipped the
            // goal when there is none), so only the cheap order flag is read here.
            let w = world.config.gangs.order_weight;
            let following = crate::systems::gang::following_order(world, id).is_some();
            let mut cs = vec![
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.5)),
                Consideration::new("target", 1.0, GATE),
                Consideration::new(
                    "order",
                    if following { p.loyalty } else { 0.0 },
                    Curve::Linear { m: w, b: 1.0 - w },
                ),
            ];
            // L1: the target is found here (once), for its walk; a tired or
            // hungry member works less (GangWork's flat ~0.85 beat Sleep).
            let (target, _) = crate::systems::gang::gang_work_target(world, id)?;
            let tiles = if world.has::<crate::components::Building>(target) {
                crate::systems::life::tiles_to(world, id, target)
            } else {
                world
                    .comp::<crate::components::Position>(target)
                    .and_then(|tp| crate::systems::life::tiles_to_tile(world, id, tp.tile))
            };
            cs.push(Consideration::new("energy", n.energy, Curve::Logistic { k: 10.0, mid: 0.3 }));
            cs.push(Consideration::new("hunger", n.hunger, Curve::Linear { m: 0.6, b: 0.4 }));
            cs.push(crate::systems::life::travel(world, tiles.unwrap_or(0)));
            cs
        }
        GoalKind::Raid => {
            // M12 D31: a gang member's muster, or a rioter's (no loyalty bar).
            let rioter = matches!(
                crate::systems::raid::expedition_of(world, id),
                Some(crate::systems::raid::Expedition::Riot(_))
            );
            if !rioter {
                world.comp::<crate::components::GangMember>(id)?;
            }
            let n = needs?;
            let p = pers?;
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            let called = crate::systems::raid::raid_pending(world, id)
                && (rioter || p.loyalty >= world.config.gangs.freelance_loyalty);
            // Pressure pass round 2, R5 (OPEN-214): the posted muster is a duty.
            if called {
                flat += crate::systems::crew::muster_flat(world, id);
            }
            vec![
                Consideration::new("muster called", can(called), GATE),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("loyalty", p.loyalty, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("U(safety)", urgency(n.safety), Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.5)),
            ]
        }
        GoalKind::Bury => {
            let mem = world.comp::<Memory>(id)?;
            let n = needs?;
            let p = pers?;
            let corpse = mem.entries.iter().find(|e| {
                e.kind == MemoryKind::SawCorpse
                    && e.subject.is_some_and(|c| world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried))
            })?;
            let kin = corpse.subject.is_some_and(|c| {
                world
                    .edges
                    .get(&crate::components::edge_key(id, c))
                    .is_some_and(|e| matches!(e.kind, RelKind::Family | RelKind::Spouse | RelKind::Parent))
            });
            let digger = world.comp::<Job>(id).is_some_and(|j| j.role == Role::Gravedigger);
            // L1: only those who may carry the body score it (a bystander's
            // Bury was unplannable while a gravedigger lived, and cooled).
            if !(digger || kin || !world.workers(Role::Gravedigger).iter().any(|&c| world.has::<Brain>(c))) {
                return None;
            }
            let duty = if kin {
                1.0
            } else if digger {
                0.9
            } else {
                0.25
            };
            vec![
                Consideration::new("corpse unburied", 1.0, GATE),
                Consideration::new("duty", duty, IDENTITY),
                Consideration::new("lawfulness", p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("U(safety)", urgency(n.safety), Curve::Linear { m: -0.5, b: 1.0 }),
            ]
        }
        GoalKind::Idle => vec![Consideration::new("constant", 0.0, Curve::Step { t: 0.0, lo: 0.05, hi: 0.05 })],
        // M12 D27 (spec § 4). `already_satisfied` has just run the
        // eligibility check (a derelict slot within reach, not banned, no bed
        // affordable). Plan deviation: the wealth term reads `U(wealth)`, not
        // the spec's `1 − U(wealth)`, so the poor squat more, not less.
        GoalKind::Squat => {
            let n = needs?;
            let p = pers?;
            let mut cs = vec![
                Consideration::new("derelict slot", can(true), GATE),
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.4, b: 0.6 }),
                Consideration::new("night", can(phase == DayPhase::Night), Curve::Step { t: 1.0, lo: 0.3, hi: 1.0 }),
            ];
            let tiles = crate::systems::street::squat_target(world, id)
                .and_then(|b| crate::systems::life::tiles_to(world, id, b))
                .unwrap_or(0);
            cs.push(crate::systems::life::travel(world, tiles));
            cs
        }
        // M13 D34 (spec § 4 table): `max(edgy − sanity, addiction − hooked
        // + 0.2)`; D39: the price is Detox's when the addiction term drives
        // it. `already_satisfied` has skipped the calm and the clean.
        GoalKind::Treat => {
            let n = needs?;
            let p = pers?;
            // The pressure pass phase 2 (care-seeking): a fresh wound, or a
            // cancer the Clinic is out of reach of the pool, seen to at a
            // Clinic or the back room (`customers::care_plan`).
            if !crate::systems::health::wants_oncology(world, id) {
                if let Some((_, what, _)) = crate::systems::customers::care_choice(world, id) {
                    let x = match what {
                        crate::systems::customers::Care::Onco | crate::systems::customers::Care::Clinic => {
                            world.comp::<crate::components::Health>(id).and_then(|h| h.cancer).map_or(0.0, |c| c.stage)
                        }
                        crate::systems::customers::Care::Patch => 1.0 - n.safety,
                    };
                    return Some((
                        vec![
                            Consideration::new("a doctor within reach, can pay", 1.0, GATE),
                            Consideration::new("hurt or stage", x, Curve::Linear { m: 0.6, b: 0.4 }),
                            Consideration::new("energy", n.energy, Curve::Linear { m: 0.4, b: 0.6 }),
                        ],
                        0.0,
                    ));
                }
            }
            // Hunger as sickness § 3: a diagnosed cancer the pool can pay for.
            if crate::systems::health::wants_oncology(world, id) {
                let stage = world.comp::<crate::components::Health>(id).and_then(|h| h.cancer).map_or(0.0, |c| c.stage);
                return Some((
                    vec![
                        Consideration::new("a Clinic, the pool pays", 1.0, GATE),
                        Consideration::new("cancer stage", stage, Curve::Linear { m: 0.6, b: 0.4 }),
                        Consideration::new("lawfulness", p.lawfulness, Curve::Linear { m: 0.2, b: 0.8 }),
                    ],
                    0.0,
                ));
            }
            let body = world.comp::<crate::components::Body>(id)?;
            let tile = world.comp::<crate::components::Position>(id)?.tile;
            let clinic = crate::systems::assets::nearest_seller(world, BuildingKind::Clinic, tile, true);
            let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
            let detox = crate::systems::stims::detox_drives(world, id);
            let ok = clinic.is_some_and(|c| {
                let price = if detox {
                    crate::systems::stims::detox_price(world, c)
                } else {
                    crate::systems::chrome::therapy_price(world, c)
                };
                coins >= price
            });
            let sanity_x = (world.config.chrome.edgy - body.sanity).clamp(0.0, 1.0);
            // A branch: a body with no addiction reads the phase 3 term alone.
            let x = if body.addiction > 0.0 {
                sanity_x.max((body.addiction - world.config.stims.hooked + 0.2).clamp(0.0, 1.0))
            } else {
                sanity_x
            };
            vec![
                Consideration::new("clinic open, can pay", can(ok), GATE),
                Consideration::new("edgy - sanity", x, Curve::Logistic { k: 8.0, mid: 0.3 }),
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("lawfulness", p.lawfulness, Curve::Linear { m: 0.4, b: 0.6 }),
            ]
        }
        // M13 D35: `already_satisfied` has found a body within reach.
        GoalKind::Loot => {
            let n = needs?;
            let p = pers?;
            let tile = world.comp::<crate::components::Position>(id)?.tile;
            let cover = world.district(world.district_of(tile)).coverage;
            vec![
                Consideration::new("body within reach", can(true), GATE),
                Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-U(wealth)", 1.0 - urgency(n.wealth), Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-coverage/2", 1.0 - cover / 2.0, Curve::Linear { m: 0.5, b: 0.5 }),
            ]
        }
        // M13 D39 (spec § 4 table): `already_satisfied` has opened the gate
        // (a dose in hand, or a source within reach and the coins for it).
        GoalKind::GetHigh => {
            let n = needs?;
            let p = pers?;
            world.comp::<crate::components::Body>(id)?;
            let on_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            let tired = if on_shift { 1.0 - n.energy } else { 0.0 };
            flat = world.config.stims.get_high_flat;
            let mut cs = vec![
                Consideration::new("dose or source", can(true), GATE),
                Consideration::new(
                    "craving",
                    crate::systems::stims::craving(world, id),
                    Curve::Logistic { k: 8.0, mid: 0.4 },
                ),
                Consideration::new("1-energy on shift", tired, Curve::Linear { m: 0.5, b: 0.2 }),
                Consideration::new("1-mood", 1.0 - (mood + 1.0) / 2.0, Curve::Linear { m: 0.4, b: 0.4 }),
            ];
            if !crate::systems::stims::legal_reach(world, id) {
                cs.push(Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.5, b: 0.5 }));
            }
            // L1: a dose in hand is free; a dealer across town is a walk.
            let held = world.comp::<Inventory>(id).is_some_and(|i| i.stims > 0);
            let tiles = if held { 0 } else { crate::systems::stims::source_tiles(world, id).unwrap_or(0) };
            let t = crate::systems::life::travel(world, tiles);
            // The walk weighs on the flat too: it alone beat Idle, so
            // every idle hour with coins went to a dealer across town.
            flat *= t.output;
            cs.push(t);
            cs
        }
        // M15 W20 (`think` floors it at `hold_score` for a hunter).
        GoalKind::Hunt => return crate::systems::hunt::considerations(world, id),
        // M16a (plan C13): work a taken record, or network at a Fixer.
        GoalKind::Contract => return crate::systems::contracts::considerations(world, id),
        // The Hit market (H9): walk to the Fixer with a want.
        GoalKind::Commission => return crate::systems::hitmarket::commission_considerations(world, id),
        // Jobs & lives phase 2: one's Pitch's hours, or the dawn corner.
        GoalKind::Pitch => return crate::systems::streetwork::considerations(world, id),
        // M16b phase 3: carry the downed to a Clinic.
        GoalKind::Rescue => return crate::systems::wounded::considerations(world, id),
        // Lives at work phase 2 (§ 3.5): a visit due to a hurt kin or an elder parent.
        GoalKind::Visit => return crate::systems::household::considerations(world, id),
        // L2 L14, L20.
        GoalKind::Unwind => {
            let (mut cs, flat) = crate::systems::leisure::considerations(world, id)?;
            // Hunger fix item 12 (P5j): nearing a work night's bedtime (or,
            // up and rested, the morning's leaving time), Unwind weighs
            // less, down to `work_night_unwind_mult` at it and past it.
            let p = crate::exec::routine::work_night_pressure(world, id);
            if p > 0.0 {
                let m = 1.0 - (1.0 - world.config.life.work_night_unwind_mult.clamp(0.01, 1.0)) * p;
                cs.push(Consideration::raw("a work night's bedtime", p, m));
            }
            // Pressure pass F3: a skipped shift's hours go somewhere (a bar,
            // a friend's, the street), not to the empty day.
            return Some((cs, flat + crate::systems::misery::skip_flat(world, id)));
        }
        GoalKind::Lead => return crate::systems::leisure::lead_considerations(world, id),
        // M15 W35.
        GoalKind::GuardBody => return crate::systems::grudges::guard_body_considerations(world, id),
        // M14 V29: the best run's considerations (`think` scores it from
        // its offer; this arm serves other readers).
        GoalKind::Hack => {
            let o = crate::systems::virt::hack_choice(world, id)?;
            flat = world.config.hack.hack_flat;
            o.considerations
        }
        // M13 D43: the best affordable offer's considerations (spec § 6).
        GoalKind::Shop => {
            let o = crate::systems::assets::shop_choice(world, id, true)?;
            flat = world.config.shop.shop_flat;
            o.considerations
        }
        // M11 § 6 / D26: open a business. `already_satisfied` has just run
        // the eligibility check (it skips Found for the ineligible).
        GoalKind::Found => {
            let n = needs?;
            let p = pers?;
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            flat = world.config.corps.found_flat;
            vec![
                Consideration::new("can found", can(true), GATE),
                Consideration::new("greed", p.greed, SQUARE),
                // M11 phase 5b: spec Linear{0.3, 0.7} read a rich agent's
                // low wealth urgency as 0.7, so the richer the founder the
                // less they founded; inverted, savings push toward founding.
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: -0.3, b: 1.0 }),
                Consideration::new("lawfulness", p.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.3)),
            ]
        }
    };
    Some((cs, flat))
}

/// L2 shadow fixes item 3: below `[life] energy_brake` a discretionary
/// goal weighs `0.5 + energy` (energy 0: half; at the brake: 1). `None`
/// with the fixes off or above the brake.
pub fn energy_brake(world: &World, n: &Needs) -> Option<Consideration> {
    if n.energy >= world.config.life.energy_brake {
        return None;
    }
    Some(Consideration::raw("energy brake", n.energy, (0.5 + n.energy).min(1.0)))
}

/// Chat is allowed at the Market, Bar, Farm and one's own Home.
pub fn chat_venue(world: &World, id: EntityId) -> bool {
    use crate::components::{Building, BuildingKind, Household, Position};
    let Some(b) = world.comp::<Position>(id).and_then(|p| p.building) else { return false };
    let own_home = world.comp::<Household>(id).and_then(|h| h.home) == Some(b);
    own_home
        || world
            .comp::<Building>(b)
            .is_some_and(|bd| matches!(bd.kind, BuildingKind::Market | BuildingKind::Bar | BuildingKind::Farm))
}

/// Shared by every goal that asks.
pub fn has_spouse(world: &World, id: EntityId) -> bool {
    crate::systems::social::has_spouse(world, id)
}

/// An Enemy-edge partner (or the subject of a WasRobbed memory) within `r` tiles.
pub fn hostile_near(world: &World, id: EntityId, r: u32) -> bool {
    world.enemies_of(id).any(|o| world.has::<Brain>(o) && crate::systems::law::near(world, id, o, r))
}

/// Did `other` wrong `id` personally: a WasRobbed memory naming them with no
/// fight since? Fights are revenge, not vigilantism: witnessing someone's
/// crime makes them an enemy, but only their victims attack, and one fight
/// settles a robbery (M8: before, a Fought memory itself counted as a wrong,
/// so every robbery became a month-long feud). A rival gang's member counts
/// too: the turf is the grievance.
pub fn wronged_by(world: &World, id: EntityId, other: EntityId) -> bool {
    let rival = match (world.gang_of(id), world.gang_of(other)) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    rival
        // M15 W26: the actor of a refused shakedown, for the backlash hour.
        || crate::systems::moves::fight_bonus_on(world, id, other).is_some()
        // M15 W17: with the legacy second-hand copies retired, a grudge of
        // `fight_grudge_min` or more is the wrong.
        || crate::systems::grudges::holds(world, id, other, world.config.grudges.fight_grudge_min)
        || world.comp::<Memory>(id).is_some_and(|m| {
            let about = |e: &&crate::components::MemoryEntry| e.subject == Some(other);
            let avenged = m.entries.iter().filter(about).filter(|e| e.kind == MemoryKind::Fought).map(|e| e.tick).max();
            m.entries
                .iter()
                .filter(about)
                .any(|e| e.kind == MemoryKind::WasRobbed && avenged.is_none_or(|t| e.tick > t))
        })
}

/// An enemy within `r` tiles who wronged this agent and was not fought in
/// the last two days.
pub fn fresh_hostile_near(world: &World, id: EntityId, r: u32) -> bool {
    fight_target_within(world, id, r).is_some()
}

/// Fight's target (the plan binds it): the hostile within 4 tiles, a robber
/// first, then the nearest.
pub fn fight_target(world: &World, id: EntityId) -> Option<EntityId> {
    fight_target_within(world, id, 4)
}

/// An enemy within `r` tiles who wronged this agent, was not fought in the
/// last two days and (Shadow V4 bug 3) is not lying downed: a robber first,
/// then the nearest (ties the lower id).
fn fight_target_within(world: &World, id: EntityId, r: u32) -> Option<EntityId> {
    let tile = world.comp::<crate::components::Position>(id)?.tile;
    let two_days = world.tick.saturating_sub(2 * time::TICKS_PER_DAY);
    let mem = world.comp::<Memory>(id);
    let fought = |o: EntityId| {
        mem.is_some_and(|m| {
            m.entries.iter().any(|e| e.kind == MemoryKind::Fought && e.subject == Some(o) && e.tick >= two_days)
        })
    };
    let robbed_by = |o: EntityId| {
        mem.is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::WasRobbed && e.subject == Some(o)))
    };
    world
        .enemies_of(id)
        .filter(|&o| {
            world.has::<Brain>(o)
                && crate::systems::law::near(world, id, o, r)
                && !crate::systems::wounded::is_down(world, o)
                && !fought(o)
                && wronged_by(world, id, o)
        })
        .min_by_key(|&o| {
            (
                !robbed_by(o),
                world.comp::<crate::components::Position>(o).map_or(u32::MAX, |p| p.tile.manhattan(tile)),
                o.index,
            )
        })
}

/// Shadow V4 bug 3: the agent's share of the two sides' strength
/// (`raid::strength` times the frailty and maims of `ages::work_mult`).
pub fn fight_odds(world: &World, id: EntityId, other: EntityId) -> f32 {
    let s =
        |a: EntityId| crate::systems::raid::strength(world, a).max(0.01) * crate::systems::ages::work_mult(world, a);
    let (me, them) = (s(id), s(other));
    me / (me + them).max(0.01)
}
