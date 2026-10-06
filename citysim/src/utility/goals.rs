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
pub const GOAL_ORDER: [GoalKind; 23] = [
    GoalKind::Eat,
    GoalKind::Sleep,
    GoalKind::Work,
    GoalKind::Earn,
    GoalKind::Socialise,
    GoalKind::Court,
    GoalKind::Flee,
    GoalKind::Fight,
    GoalKind::ReportCrime,
    GoalKind::Patrol,
    GoalKind::Arrest,
    GoalKind::JoinGang,
    GoalKind::GangWork,
    GoalKind::Raid,
    GoalKind::Bury,
    // M12 D27: before Found.
    GoalKind::Squat,
    // M13 D47: before Found, in the order Shop, GetHigh, Treat, Loot.
    GoalKind::Shop,
    GoalKind::GetHigh,
    GoalKind::Treat,
    GoalKind::Loot,
    // M14 V41: before Found.
    GoalKind::Hack,
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
        GoalKind::Eat => needs.is_some_and(|n| n.hunger >= HUNGER_SATISFIED),
        GoalKind::Sleep => needs.is_some_and(|n| n.energy >= ENERGY_SATISFIED),
        GoalKind::Socialise => {
            // Until Chat arrives (M5) a drink is the only satisfier: the goal is
            // off for the day once had, and off while it cannot be had (no 2
            // coins, or the Bar is visibly full). Otherwise it wins every think
            // and churns through plan failures.
            // Satisfied, or nothing to do for it: no Chat partner here and no drink possible.
            let chat_possible = chat_venue(world, id)
                && crate::systems::social::best_colocated_partner(world, id, -1.0, false).is_some();
            needs.is_some_and(|n| n.belonging >= BELONGING_SATISFIED)
                || (!chat_possible
                    && (world.comp::<Wallet>(id).is_some_and(|w| w.coins < 2)
                        || bar_full_for(world, id)
                        || drank_today(world, id)))
        }
        GoalKind::Flee => needs.is_some_and(|n| n.safety >= SAFE),
        GoalKind::Earn => {
            let price = world.local(id, BuildingKind::Market).map_or(i64::MAX, |m| world.price_for(m, id));
            if world.comp::<Wallet>(id).is_some_and(|w| w.coins >= SAVINGS_DAYS.saturating_mul(price)) {
                return true;
            }
            // Nothing to plan: an unemployed agent whose dole is taken or
            // suspended (Beg yields coins, never savings), or an employed one
            // off shift with no wage due (the in-shift case stays scored, as in
            // the spec's worked example; it loses to Work's hysteresis).
            let day = world.day();
            let nothing = match world.comp::<Job>(id) {
                Some(j) => j.days_unpaid == 0 && !j.on_shift(world.tick_of_day()),
                None => {
                    world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day == Some(day))
                        || world.treasury().is_some_and(|t| t.coins < 0)
                        || world.levers.dole_per_day == 0
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
                || world.comp::<Brain>(id).is_some_and(|b| b.gang_task_day == Some(world.day()))
                || crate::systems::gang::extort_target(world, id).is_none()
        }
        // A guard never scores JoinGang (`considerations` says None): skip the scan.
        GoalKind::JoinGang => {
            world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard) || !crate::systems::gang::eligible(world, id)
        }
        // M12 D31: a rioter has an expedition too (raid_done is true without one).
        GoalKind::Raid => crate::systems::raid::raid_done(world, id),
        GoalKind::Work => world.comp::<Job>(id).is_some_and(|j| {
            j.last_shift_day == Some(j.next_shift_key(world.tick)) && !crate::exec::routine::wage_pending(world, j)
        }),
        // M11 D26: nothing to found (also the eligibility gate).
        GoalKind::Found => !crate::systems::founding::can_found(world, id),
        // M12 D27: squatting already, or nowhere to squat (the eligibility gate).
        GoalKind::Squat => !crate::systems::street::can_squat(world, id),
        // M13 D43: nothing affordable, on cooldown, in arrears or no open seller.
        GoalKind::Shop => crate::systems::assets::shop_choice(world, id, true).is_none(),
        // M13 D34: nothing to treat at `treat_below`; D39: and no addiction
        // within 0.2 of `hooked` to Detox.
        GoalKind::Treat => {
            !crate::systems::chrome::wants_treatment(world, id) && !crate::systems::stims::wants_detox(world, id)
        }
        // M13 D39: high already, or no dose in hand and none to buy.
        GoalKind::GetHigh => !crate::systems::stims::wants_high(world, id),
        // M13 D35: no body within reach to strip.
        GoalKind::Loot => crate::systems::chrome::loot_target(world, id).is_none(),
        // M14 V29: no deck, chair or target worth the risk.
        GoalKind::Hack => crate::systems::virt::hack_choice(world, id).is_none(),
        _ => false,
    }
}

/// Considerations and flat bonus for one goal, or `None` if the agent lacks a
/// required component.
pub fn considerations(
    world: &World,
    id: EntityId,
    goal: GoalKind,
    has_spouse: bool,
) -> Option<(Vec<Consideration>, f32)> {
    let needs = world.comp::<Needs>(id);
    let pers = world.comp::<Personality>(id);
    let mood = world.comp::<crate::components::Mood>(id).map_or(0.0, |m| m.value);
    let phase = world.phase();
    let tod = world.tick_of_day();
    let mm = mood_mult(mood, goal);

    let mut flat = 0.0;
    let cs = match goal {
        GoalKind::Eat => {
            let n = needs?;
            let inv = world.comp::<Inventory>(id)?;
            let wallet = world.comp::<Wallet>(id)?;
            let pantry = world
                .comp::<Household>(id)
                .and_then(|h| h.home)
                .and_then(|h| world.comp::<crate::components::Building>(h))
                .map_or(0, |b| b.stock_food);
            let lawfulness = pers.map_or(0.5, |p| p.lawfulness);
            // Only Eat reads the local price: one Market scan per think, not per goal.
            let price = world.local(id, BuildingKind::Market).map_or(i64::MAX, |m| world.price_for(m, id));
            let can_eat = inv.food > 0 || wallet.coins >= price || pantry > 0 || lawfulness < 0.3;
            vec![
                Consideration::new("U(hunger)", urgency(n.hunger), SQUARE),
                Consideration::new("can afford/obtain", can(can_eat), GATE),
            ]
        }
        GoalKind::Sleep => {
            let n = needs?;
            let lo = match phase {
                DayPhase::Night => 1.0,
                DayPhase::Evening => 0.7,
                _ => 0.4,
            };
            vec![
                Consideration::new("U(energy)", urgency(n.energy), Curve::Logistic { k: 10.0, mid: 0.75 }),
                Consideration::raw("phase", can(phase == DayPhase::Night), lo),
            ]
        }
        GoalKind::Work => {
            let job = world.comp::<Job>(id)?;
            let n = needs?;
            if world.has::<Sentence>(id) {
                return None;
            }
            vec![
                Consideration::new("in shift", can(shift_reachable(world, id, job)), GATE),
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
            ]
        }
        GoalKind::Earn => {
            let n = needs?;
            world.comp::<Wallet>(id)?;
            world.comp::<Skills>(id)?;
            let p = pers?;
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            vec![
                Consideration::new("U(wealth)", urgency(n.wealth), SQUARE),
                Consideration::new("U(hunger)", urgency(n.hunger), IDENTITY),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.3)),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.5, b: 0.5 }),
            ]
        }
        GoalKind::Socialise => {
            let n = needs?;
            let p = pers?;
            let mood_x = (mood + 1.0) / 2.0;
            vec![
                Consideration::new("U(belonging)", urgency(n.belonging), Curve::Logistic { k: 6.0, mid: 0.5 }),
                Consideration::raw("mood", mood_x, IDENTITY.eval(mood_x) * mm),
                Consideration::new("sociability", p.sociability, IDENTITY),
                phase_curve(phase, &[DayPhase::Evening], 0.4),
            ]
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
            let candidate = world.comp::<Brain>(id).and_then(|b| b.court_candidate).is_some_and(|(_, c)| c)
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
            let fresh_enemy = fresh_hostile_near(world, id, 4);
            let robbed_by_enemy = mem.is_some_and(|m| {
                m.entries.iter().any(|e| {
                    e.kind == MemoryKind::WasRobbed
                        && e.subject.is_some_and(|s| {
                            world.enemies_of(id).any(|o| o == s) && crate::systems::law::near(world, id, s, 4)
                        })
                })
            });
            if robbed_by_enemy {
                flat += 0.2;
            }
            vec![
                Consideration::new("enemy near", can(fresh_enemy), GATE),
                Consideration::new("courage", p.courage, IDENTITY),
                Consideration::new("fighting", s.fighting, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("U(safety)", urgency(n.safety), Curve::Linear { m: 0.5, b: 0.5 }),
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
            vec![
                Consideration::new("lawfulness", p.lawfulness, Curve::Logistic { k: 8.0, mid: 0.5 }),
                Consideration::new("salience", saw.salience, IDENTITY),
                Consideration::new("hall open", can(hall_open), GATE),
                Consideration::new("1-affinity(suspect)", (1.0 - affinity).clamp(0.0, 1.0), IDENTITY),
            ]
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
            let on_duty = job.on_shift(tod) && patrol_day && job.last_shift_day != Some(key) && legs_left;
            // The same reach as Arrest's gate (M10 phase 5c review): a
            // located warrant across the city scaled every guard's Patrol by
            // 0.3 while only the guards in range could Arrest, so the rest
            // idled, never walked a leg and were never owed the shift.
            let here = world.comp::<crate::components::Position>(id).map(|p| p.tile).unwrap_or_default();
            let by = crate::systems::law::Pursuer { tile: here, guard: id };
            let no_warrant = !crate::systems::law::any_located_suspect(world, Some(by));
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
            let located = crate::systems::law::any_located_suspect(world, Some(by))
                || world.comp::<Brain>(id).is_some_and(|b| b.escorting.is_some());
            vec![
                Consideration::new("warrant located", can(located), GATE),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("in shift", can(job.on_shift(tod)), gate_or(0.5)),
            ]
        }
        GoalKind::JoinGang => {
            if world.has::<crate::components::GangMember>(id)
                || world.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)
            {
                return None;
            }
            let n = needs?;
            let p = pers?;
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
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            // The order tilts the day: a loyal member following orders scores
            // 1.0 here, a freelancer 1 - order_weight. Hunger and fear still win.
            // `already_satisfied` has just scanned for a target (and skipped the
            // goal when there is none), so only the cheap order flag is read here.
            let w = world.config.gangs.order_weight;
            let following = crate::systems::gang::following_order(world, id).is_some();
            vec![
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.5)),
                Consideration::new("target", 1.0, GATE),
                Consideration::new(
                    "order",
                    if following { p.loyalty } else { 0.0 },
                    Curve::Linear { m: w, b: 1.0 - w },
                ),
            ]
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
            vec![
                Consideration::new("derelict slot", can(true), GATE),
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("1-lawfulness", 1.0 - p.lawfulness, Curve::Linear { m: 0.4, b: 0.6 }),
                Consideration::new("night", can(phase == DayPhase::Night), Curve::Step { t: 1.0, lo: 0.3, hi: 1.0 }),
            ]
        }
        // M13 D34 (spec § 4 table): `max(edgy − sanity, addiction − hooked
        // + 0.2)`; D39: the price is Detox's when the addiction term drives
        // it. `already_satisfied` has skipped the calm and the clean.
        GoalKind::Treat => {
            let n = needs?;
            let p = pers?;
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
            cs
        }
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
    let two_days = world.tick.saturating_sub(2 * time::TICKS_PER_DAY);
    let mem = world.comp::<Memory>(id);
    world.enemies_of(id).any(|o| {
        world.has::<Brain>(o)
            && crate::systems::law::near(world, id, o, r)
            && wronged_by(world, id, o)
            && !mem.is_some_and(|m| {
                m.entries.iter().any(|e| e.kind == MemoryKind::Fought && e.subject == Some(o) && e.tick >= two_days)
            })
    })
}
