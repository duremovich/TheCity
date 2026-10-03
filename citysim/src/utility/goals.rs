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
pub const GOAL_ORDER: [GoalKind; 15] = [
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
    GoalKind::Bury,
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

/// The Bar is at capacity and this agent is not inside it.
pub fn bar_full_for(world: &World, id: EntityId) -> bool {
    world
        .building_of_kind(BuildingKind::Bar)
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
            needs.is_some_and(|n| n.belonging >= BELONGING_SATISFIED)
                || world.comp::<Wallet>(id).is_some_and(|w| w.coins < 2)
                || bar_full_for(world, id)
                || drank_today(world, id)
        }
        GoalKind::Flee => needs.is_some_and(|n| n.safety >= SAFE),
        GoalKind::Earn => {
            let price = world.market().map_or(i64::MAX, |m| m.price_food);
            if world.comp::<Wallet>(id).is_some_and(|w| w.coins >= SAVINGS_DAYS.saturating_mul(price)) {
                return true;
            }
            // Nothing to plan: an unemployed agent whose dole is taken or
            // suspended (Beg yields coins, never savings), or an employed one
            // off shift with no wage due (the in-shift case stays scored, as in
            // the spec's worked example; it loses to Work's hysteresis).
            let day = world.day();
            match world.comp::<Job>(id) {
                Some(j) => j.days_unpaid == 0 && !j.on_shift(world.tick_of_day()),
                None => {
                    world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day == Some(day))
                        || world.treasury().is_some_and(|t| t.coins < 0)
                        || world.levers.dole_per_day == 0
                }
            }
        }
        GoalKind::Court => has_spouse,
        GoalKind::Work => world.comp::<Job>(id).is_some_and(|j| {
            j.last_shift_day == Some(j.next_shift_key(world.tick)) && !crate::exec::routine::wage_pending(world, j)
        }),
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
    let price = world.market().map_or(i64::MAX, |m| m.price_food);
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
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 8.0, mid: 0.5 }),
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
            let candidate = world.edges.iter().any(|(&(a, b), e)| (a == id || b == id) && e.affinity >= 0.3);
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
            vec![
                Consideration::new("U(safety)", urgency(n.safety), Curve::Logistic { k: 12.0, mid: 0.5 }),
                // A guard is hostile to the wanted; enemies arrive with the social graph (M5).
                Consideration::new(
                    "hostile near",
                    can(crate::systems::law::wanted(world, id)
                        && crate::systems::law::guard_within(world, id, world.config.crime.sight)),
                    GATE,
                ),
                Consideration::new("1-courage", 1.0 - p.courage, Curve::Linear { m: 0.8, b: 0.2 }),
            ]
        }
        GoalKind::Fight => {
            let n = needs?;
            let s = world.comp::<Skills>(id)?;
            let p = pers?;
            if mood < -0.6 {
                flat = 0.15;
            }
            vec![
                Consideration::new("enemy near", can(false), GATE), // M5: rivals/enemies
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
                .filter(|e| e.subject.is_some_and(|s| !crate::systems::law::wanted(world, s)))
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
            let patrol_day = !crate::systems::law::jail_day(id, key);
            let legs_left =
                world.comp::<Brain>(id).is_some_and(|b| b.patrol_legs < world.config.crime.patrol_legs_per_shift);
            let on_duty = job.on_shift(tod) && patrol_day && job.last_shift_day != Some(key) && legs_left;
            let no_warrant = crate::systems::law::located_suspects(world).is_empty();
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
            let located = !crate::systems::law::located_suspects(world).is_empty()
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
                Consideration::new("eligible", can(false), GATE), // M5: Gang › Eligibility
                Consideration::new("courage", p.courage, Curve::Linear { m: 0.5, b: 0.5 }),
            ]
        }
        GoalKind::GangWork => {
            world.comp::<crate::components::GangMember>(id)?;
            let n = needs?;
            let p = pers?;
            let in_shift = world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod));
            vec![
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("greed", p.greed, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("not in shift", can(!in_shift), gate_or(0.5)),
                Consideration::new("no guard near target", can(false), gate_or(0.2)), // M5
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
    };
    Some((cs, flat))
}

/// One scan of the edge map per think, shared by every goal that asks.
pub fn has_spouse(world: &World, id: EntityId) -> bool {
    world.edges.iter().any(|(&(a, b), e)| e.kind == RelKind::Spouse && (a == id || b == id))
}
