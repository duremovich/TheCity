//! Mood: a slow blend of need satisfaction and recent memories, computed
//! once per hour for every agent.

use crate::components::{Memory, Mood, Needs, Personality};
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
        .map(|(e, age)| e.valence * e.salience * 0.5f32.powf(age / 3.0))
        .sum();
    sum.clamp(-1.0, 1.0)
}

/// One hourly update of an agent's mood. Returns the new value.
pub fn update(
    mood: &mut Mood,
    needs: &Needs,
    memory: &Memory,
    pride: f32,
    w_need: f32,
    w_memory: f32,
    now: Tick,
) -> f32 {
    let raw = (w_need * need_term(needs) + w_memory * memory_term(memory, now)).clamp(-1.0, 1.0);
    let mut delta = 0.2 * (raw - mood.value);
    if delta < 0.0 {
        delta *= 1.0 + 0.5 * pride; // the proud take setbacks harder
    }
    mood.value = (mood.value + delta).clamp(-1.0, 1.0);
    mood.last_computed = now;
    if mood.value < -0.8 {
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

fn update_agent(world: &mut World, id: EntityId, w_need: f32, w_memory: f32, now: Tick) {
    let (Some(needs), Some(memory)) = (world.comp::<Needs>(id).cloned(), world.comp::<Memory>(id).cloned()) else {
        return;
    };
    let pride = world.comp::<Personality>(id).map_or(0.5, |p| p.pride);
    if let Some(mood) = world.comp_mut::<Mood>(id) {
        update(mood, &needs, &memory, pride, w_need, w_memory, now);
    }
}
