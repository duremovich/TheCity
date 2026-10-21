//! The pre-view pass phase 4 (docs/PREVIEW_PASS.md § 0 P3, P5, P8; § 2
//! "settlement by presence"; OPEN-119, OPEN-120, OPEN-154): **an encounter
//! posts when, and only when, both bodies are present**. Under `[presence]`
//! (`None` with `CITYSIM_PRESENCE_OFF=1`, every entry point the old path).
//!
//! A Full or Coarse party is present at a place when its body stands in it
//! and is awake ([`body_at`]). A Statistical party has no body: it is drawn
//! present by a purpose-keyed draw consistent with where its schedule says
//! it is ([`stat_home`]: not on its shift, not the night 00:00-06:00, then
//! `stat_home_p`). An encounter whose counterpart is absent posts nothing in
//! either diary: it waits for a later meeting (the next round, the next hour
//! at the pitch, the dealer's next hour at the Bar) or lapses, counted in
//! `presence_missed` (the shadow tool narrates such a party `[off]`).
//!
//! **Statistical mirror** (doctrine 7): a Statistical-only pair keeps the
//! settle it had (the midnight arms sale, the pimp's cut at the day's close,
//! the super's first-hour stops served on the draw); a mixed pair meets on
//! the body's real presence and the Statistical party's draw.

use crate::components::{Brain, Job, Lod, Position, Sentence};
use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::goap::ActionKind;
use crate::world::World;

/// The draw purposes (`econ::hash_unit`, unique to their sites).
pub const PURPOSE_HOME: u64 = 0x5052_0001;

/// `[presence]` is on (not `CITYSIM_PRESENCE_OFF`).
pub fn on(world: &World) -> bool {
    world.config.presence.is_some()
}

/// Is `id` asleep (its body in a `Sleep`)?
pub fn asleep(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| matches!(b.exec, ExecState::Use { kind: ActionKind::Sleep, .. }))
}

/// Is `id` a body (Full or Coarse)?
pub fn is_body(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
}

/// A body standing inside `place`, awake, free and alive.
pub fn body_at(world: &World, id: EntityId, place: EntityId) -> bool {
    world.comp::<Position>(id).is_some_and(|p| p.building == Some(place))
        && !asleep(world, id)
        && !world.has::<Sentence>(id)
        && crate::systems::law::living(world, id)
}

/// A Statistical resident drawn at home and awake now: not on its shift
/// (attended), not the night (00:00-06:00, asleep), then `stat_home_p` on a
/// draw keyed by the purpose, the agent, `key` and the hour.
pub fn stat_home(world: &World, id: EntityId, purpose: u64, key: u64) -> bool {
    let Some(cfg) = world.config.presence.as_ref() else { return true };
    if world.has::<Sentence>(id) || !crate::systems::law::living(world, id) {
        return false;
    }
    if u64::from(world.tick_of_day()) < 6 * crate::time::TICKS_PER_HOUR {
        return false;
    }
    if let Some(j) = world.comp::<Job>(id) {
        let at_work = j.employer.is_some()
            && j.on_shift(world.tick_of_day())
            && crate::exec::routine::workday_of(world, id, j, j.shift_key_at(world.tick))
            && crate::systems::lod::stat_attend(world, id, j.role)
                .is_none_or(|p| crate::systems::lod::shift_attended(world, id, j, p));
        if at_work {
            return false;
        }
    }
    let hour = world.tick / crate::time::TICKS_PER_HOUR;
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(id.index) ^ key.rotate_left(20), hour)
        < cfg.stat_home_p
}

/// Is `id` met at its own Home `place` now: its body there awake, or, a
/// Statistical resident, the [`stat_home`] draw.
pub fn met_at_home(world: &World, id: EntityId, place: EntityId, key: u64) -> bool {
    match world.comp::<Brain>(id).map(|b| b.lod) {
        Some(Lod::Statistical) => stat_home(world, id, PURPOSE_HOME, key),
        Some(_) => body_at(world, id, place),
        None => false,
    }
}

/// One encounter skipped for an absent counterpart (`presence_missed`).
pub fn missed(world: &mut World) {
    crate::systems::ownership::jobs2_row(world).presence_missed += 1;
}
