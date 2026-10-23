//! Mood: a slow blend of need satisfaction and recent memories, computed
//! once per hour for every agent.

use crate::components::{Memory, MemoryKind, Mood, Needs, Personality};
use crate::entity::EntityId;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// `need_term` from the six needs: `0.30·(2h−1) + 0.15·(2e−1) + 0.20·(2s−1)
/// + 0.15·(2w−1) + 0.12·(2b−1) + 0.08·(2i−1)`.
pub fn need_term(n: &Needs) -> f32 {
    let f = |x: f32| 2.0 * x - 1.0;
    0.30 * f(n.hunger)
        + 0.15 * f(n.energy)
        + 0.20 * f(n.safety)
        + 0.15 * f(n.wealth)
        + 0.12 * f(n.belonging)
        + 0.08 * f(n.intimacy)
}

/// `clamp(Σ valence·salience·0.5^(age_days/3) over memories ≤ 7 days old, −1, 1)`.
pub fn memory_term(m: &Memory, now: Tick) -> f32 {
    let sum: f32 = m
        .entries
        .iter()
        .map(|e| (e, now.saturating_sub(e.tick) as f32 / TICKS_PER_DAY as f32))
        .filter(|(_, age)| *age <= 7.0)
        // Lives at work D5: a `Served` memory has mood weight 0 (parity-neutral).
        .filter(|(e, _)| e.kind != MemoryKind::Served)
        .map(|(e, age)| e.valence * e.salience * 0.5f32.powf(age / 3.0))
        .sum();
    sum.clamp(-1.0, 1.0)
}

/// One hourly update of an agent's mood. Returns the new value.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mood: &mut Mood,
    needs: &Needs,
    memory: &Memory,
    pride: f32,
    w_need: f32,
    w_memory: f32,
    low_mood: f32,
    now: Tick,
) -> f32 {
    update_biased(mood, needs, memory, pride, w_need, w_memory, low_mood, now, 0.0)
}

/// `update` with a flat `bias` added to the raw target before the clamp
/// (M12 D18: `−[litter] mood × district litter`, no new Mood field).
#[allow(clippy::too_many_arguments)]
pub fn update_biased(
    mood: &mut Mood,
    needs: &Needs,
    memory: &Memory,
    pride: f32,
    w_need: f32,
    w_memory: f32,
    low_mood: f32,
    now: Tick,
    bias: f32,
) -> f32 {
    let raw = (w_need * need_term(needs) + w_memory * memory_term(memory, now) + bias).clamp(-1.0, 1.0);
    let mut delta = 0.2 * (raw - mood.value);
    if delta < 0.0 {
        delta *= 1.0 + 0.5 * pride; // the proud take setbacks harder
    }
    mood.value = (mood.value + delta).clamp(-1.0, 1.0);
    mood.last_computed = now;
    if mood.value < low_mood {
        mood.low_since.get_or_insert(now);
    } else {
        mood.low_since = None;
    }
    mood.value
}

pub fn run(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let now = world.tick;
    let w_need = world.config.brain.mood_w_need;
    let w_memory = world.config.brain.mood_w_memory;
    for id in world.citizens() {
        update_agent(world, id, w_need, w_memory, now);
    }
}

/// M12 D18: `−[litter] mood × litter` of the agent's Home district (the
/// homeless: the district it stands in); one lookup per hourly update.
fn litter_bias(world: &World, id: EntityId) -> f32 {
    if !crate::systems::litter::enabled(world) {
        return 0.0;
    }
    let at = world
        .comp::<crate::components::Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<crate::components::Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<crate::components::Position>(id).map(|p| p.tile));
    let Some(at) = at else { return 0.0 };
    -world.config.litter.mood * world.district(world.district_of(at)).litter
}

/// The island pass phase 3c (OPEN-177, `[mood.housing]`): the agent's Home
/// as a flat bias on the mood target: `tier[tier] − crowd_w × min(excess,
/// crowd_max) × factor − company`, the excess the residents a floor over
/// `[buildings] home` capacity less 1 (a plain Block 0, a full bunk floor
/// 4), the factor `inside_factor` / `below_factor` for company floors in a
/// workplace (above it / below it: the coffin racks), else 1, `company` for
/// any company bed. The homeless (and a squat, a derelict) read 0. Full and
/// Statistical alike: the Home's state, no tier-specific input.
pub fn housing_bias(world: &World, id: EntityId) -> f32 {
    let Some(cfg) = world.config.mood.housing.as_ref() else { return 0.0 };
    let Some(home) = world.comp::<crate::components::Household>(id).and_then(|h| h.home) else { return 0.0 };
    let Some(b) = world.comp::<crate::components::Building>(home) else { return 0.0 };
    if b.demolished || b.derelict {
        return 0.0;
    }
    let mut bias = cfg.tier[usize::from(b.tier.min(2))];
    let per_floor = world.residents_of(home).len() as f32 / f32::from(b.floors.max(1));
    let plain = f32::from(world.config.buildings.for_kind(crate::components::BuildingKind::Home).capacity.max(1));
    let excess = (per_floor / plain - 1.0).clamp(0.0, cfg.crowd_max);
    if excess > 0.0 {
        let below = b.within.and_then(|w| world.comp::<crate::components::Building>(w)).map(|wb| {
            world.config.company_housing.as_ref().and_then(|c| c.inside.as_ref()).is_some_and(|i| i.room(wb.kind).1)
        });
        let factor = match below {
            Some(true) => cfg.below_factor,
            Some(false) => cfg.inside_factor,
            None => 1.0,
        };
        bias -= cfg.crowd_w * excess * factor;
    }
    if crate::systems::company::of(world, home).is_some() {
        bias -= cfg.company;
    }
    bias
}

fn update_agent(world: &mut World, id: EntityId, w_need: f32, w_memory: f32, now: Tick) {
    // The small Mood is copied out instead of cloning Needs and the Memory
    // vector per citizen per hour (perf); the same update, written back.
    let (Some(needs), Some(memory)) = (world.comp::<Needs>(id), world.comp::<Memory>(id)) else {
        return;
    };
    // Jobs & lives phase 5 (§ 5.2, D9): no mood term while under a chip.
    if !world.jobs2.chip.contracts.is_empty() && crate::systems::dollhouse::under_chip(world, id) {
        return;
    }
    let Some(mut mood) = world.comp::<Mood>(id).cloned() else { return };
    let pride = world.comp::<Personality>(id).map_or(0.5, |p| p.pride);
    let low_mood = world.config.demography.emigrate_mood;
    let mut bias = litter_bias(world, id);
    // M13 D33: the body's term (a branch: a calm body adds nothing).
    let body = crate::systems::chrome::body_bias(world, id);
    if body != 0.0 {
        bias += body;
    }
    // L2 L13: `fun_mood × (fun − 0.5)` (a branch: leisure off adds nothing).
    bias += crate::systems::leisure::mood_bias(world, id);
    // Jobs & lives phase 4 (§ 9): `status_w × (status − 0.5)` (a branch: 0 off).
    let status = crate::systems::fashion::mood_bias(world, id);
    if status != 0.0 {
        bias += status;
    }
    // The island pass phase 3c (OPEN-177): the Home (a branch: 0 off).
    let housing = housing_bias(world, id);
    if housing != 0.0 {
        bias += housing;
    }
    // Hunger as sickness § 2: the body's sickness (a branch: 0 off).
    let sick = crate::systems::health::effects(world, id).mood_bias;
    if sick != 0.0 {
        bias += sick;
    }
    update_biased(&mut mood, needs, memory, pride, w_need, w_memory, low_mood, now, bias);
    if let Some(m) = world.comp_mut::<Mood>(id) {
        *m = mood;
    }
}
