//! M16a § 3 (plan C22-C25): missions and the strike estimate.
//!
//! A game abstraction: a mission is a crew list marching to a door, and its
//! outcome is the raid machinery's seeded dice (`raid::fight_out`); the
//! strike estimate is a number, `p_win = logistic(strike_k × (S_crew ÷
//! S_def − 1))` over summed `raid::strength` scores. Phase 1 lands the
//! estimate alone (`estimate`, `expected_defenders`, `MEDIAN`), which the
//! quote, `risk_for` and the ledger read; phase 2 fills the rest.

use crate::components::{Household, RelKind};
use crate::contract::Target;
use crate::entity::EntityId;
use crate::world::World;

/// C7: the phantom median gun of the quote (fighting 0.5, courage 0.5, no
/// Kit: strength 0.625); `strength_of(MEDIAN)` reads it.
pub const MEDIAN: EntityId = EntityId::NONE;

/// The median gun's strength (`0.5 + 0.25 × 0.5`).
pub const MEDIAN_STRENGTH: f32 = 0.625;

/// `raid::strength`, with the phantom median gun.
pub fn strength_of(world: &World, id: EntityId) -> f32 {
    if id == MEDIAN {
        MEDIAN_STRENGTH
    } else {
        crate::systems::raid::strength(world, id)
    }
}

/// `1 / (1 + e^−x)`.
pub fn logistic(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// C24: `p_win` from the two summed strengths.
pub fn p_win(world: &World, s_crew: f32, s_def: f32) -> f32 {
    logistic(world.config.missions.strike_k * (s_crew / s_def.max(0.1) - 1.0))
}

/// C23: the defenders a strike on `target` expects: an agent target and
/// up to `ally_cap_n` of its gang members or Friends with courage ≥ 0.5
/// living in its Home district, strongest first (ties the lower id); a
/// building target, its private and posted guards (none at HEAD's phase 1:
/// an empty list, so a Guard or Locate on a building reads its risk flat).
pub fn expected_defenders(world: &World, target: &Target) -> Vec<EntityId> {
    let Target::Agent(t) = *target else { return Vec::new() };
    if !crate::systems::law::living(world, t) {
        return Vec::new();
    }
    let home_d = world.comp::<Household>(t).and_then(|h| h.home).map(|h| world.district_of_building(h));
    let mut allies: Vec<EntityId> = Vec::new();
    if let Some(g) = world.gang_of(t).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        allies.extend(g.members.iter().copied().filter(|&m| m != t));
    }
    for o in world.neighbours(t) {
        if world.edge(t, o).is_some_and(|e| e.kind == RelKind::Friend) && !allies.contains(&o) {
            allies.push(o);
        }
    }
    let mut allies: Vec<(f32, EntityId)> = allies
        .into_iter()
        .filter(|&a| crate::systems::law::living(world, a) && crate::systems::law::courage(world, a) >= 0.5)
        .filter(|&a| !world.has::<crate::components::Sentence>(a))
        .filter(|&a| {
            home_d.is_some()
                && world.comp::<Household>(a).and_then(|h| h.home).map(|h| world.district_of_building(h)) == home_d
        })
        .map(|a| (strength_of(world, a), a))
        .collect();
    allies.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let cap = usize::from(world.config.missions.ally_cap_n);
    let mut out = vec![t];
    out.extend(allies.into_iter().take(cap).map(|(_, a)| a));
    out
}

/// C23: the strike's `p_win` for `crew` against `target`'s expected
/// defenders (the quote's, `risk_for`'s and the ledger's estimate). An
/// empty defender list (a building with no guards) reads `S_def` 0.1.
pub fn estimate(world: &World, crew: &[EntityId], target: &Target) -> f32 {
    let s_crew: f32 = crew.iter().map(|&c| strength_of(world, c)).sum();
    let s_def: f32 = expected_defenders(world, target).iter().map(|&d| strength_of(world, d)).sum();
    p_win(world, s_crew, s_def)
}

/// `estimate` against a defender strength already summed (matching scores
/// many candidates against one target).
pub fn estimate_against(world: &World, crew: &[EntityId], s_def: f32) -> f32 {
    let s_crew: f32 = crew.iter().map(|&c| strength_of(world, c)).sum();
    p_win(world, s_crew, s_def)
}

/// The summed strength of `target`'s expected defenders.
pub fn defender_strength(world: &World, target: &Target) -> f32 {
    expected_defenders(world, target).iter().map(|&d| strength_of(world, d)).sum()
}
