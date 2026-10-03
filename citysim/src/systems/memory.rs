//! Memory: a bounded, decaying list of salient events per agent.
//!
//! `remember` merges rapid duplicates and evicts the lowest
//! `salience × recency` entry past the cap; the daily decay halves salience
//! every seven days and drops entries under `MEMORY_DROP_BELOW`.

use crate::components::{Memory, MemoryEntry, MemoryKind};
use crate::entity::EntityId;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// Entries below this salience are forgotten.
pub const MEMORY_DROP_BELOW: f32 = 0.05;
/// Same kind and subject within this many ticks merge into one entry.
pub const MERGE_WINDOW: Tick = 60;

/// `salience × 0.5^(age_days / half_life)`.
pub fn weight(e: &MemoryEntry, now: Tick, half_life_days: f32) -> f32 {
    let age_days = now.saturating_sub(e.tick) as f32 / TICKS_PER_DAY as f32;
    e.salience * 0.5f32.powf(age_days / half_life_days.max(0.01))
}

/// Insert an entry into a memory with the spec's merge and eviction rules.
pub fn insert(mem: &mut Memory, entry: MemoryEntry, now: Tick, cap: usize, half_life_days: f32) {
    if let Some(e) = mem.entries.iter_mut().find(|e| {
        e.kind == entry.kind && e.subject == entry.subject && entry.tick.saturating_sub(e.tick) < MERGE_WINDOW
    }) {
        e.salience = e.salience.max(entry.salience);
        e.tick = entry.tick;
        return;
    }
    if mem.entries.len() >= cap {
        let (worst, _) = mem
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, weight(e, now, half_life_days)))
            .fold((0, f32::INFINITY), |acc, (i, w)| if w < acc.1 { (i, w) } else { acc });
        mem.entries.swap_remove(worst);
    }
    mem.entries.push(entry);
}

/// Daily decay at `tick_of_day == 0`: `salience ×= 0.5^(1/7)`; entries under
/// 0.05 are removed. Runs before any system that could hit the cap.
pub fn run(world: &mut World) {
    if world.tick_of_day() != 0 {
        return;
    }
    let half_life = world.config.brain.memory_half_life_days.max(0.01);
    let factor = 0.5f32.powf(1.0 / half_life);
    for id in world.citizens() {
        let Some(m) = world.comp_mut::<Memory>(id) else { continue };
        for e in &mut m.entries {
            e.salience *= factor;
        }
        m.entries.retain(|e| e.salience >= MEMORY_DROP_BELOW);
    }
}

/// First-hand memories of `kind` about `subject`.
pub fn has_first_hand(world: &World, who: EntityId, kind: MemoryKind, subject: Option<EntityId>) -> bool {
    world
        .comp::<Memory>(who)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == kind && e.subject == subject && !e.second_hand))
}
