//! M14 § 7: a mission watched live. A gang raid whose departure found a
//! member with a deck at the Hideout has `Gang.stream_by` set: that member
//! is jacked in on an `Overwatch` run for the raid's duration, and the
//! Mission panel shows the march and each pairing as it resolves, tagged
//! LIVE. Without a streamer the panel lists only the outcome events.
//!
//! `MissionView` is the hook M16's contracts reuse as the second of the
//! three mission renderers (M18 the third): the streamer and the bodies the
//! panel follows. It is built from sim state each frame and never saved.

use smallvec::SmallVec;

use citysim::systems::raid;
use citysim::{EntityId, Gang, GoalKind, Position, World, TICKS_PER_DAY};

/// What a live mission panel reads: who streams it and whom it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionView {
    /// The streamer (the member jacked in on `Overwatch`).
    pub source: EntityId,
    /// The marching members, ascending id (the streamer excluded).
    pub subjects: SmallVec<[EntityId; 8]>,
}

/// The gang's live view: `stream_by` set to a living runner whose run is
/// in `world.runs`. `None` is the no-streamer case.
pub fn build(world: &World, gang: EntityId) -> Option<MissionView> {
    let g = world.comp::<Gang>(gang)?;
    let source = g.stream_by?;
    let run = world.runner_of.get(&source).and_then(|id| world.runs.get(id))?;
    if !matches!(run.purpose, citysim::virt::Purpose::Overwatch(_)) {
        return None;
    }
    let mut subjects: SmallVec<[EntityId; 8]> = g
        .members
        .iter()
        .copied()
        .filter(|&m| m != source && citysim::systems::law::living(world, m))
        .filter(|&m| world.comp::<citysim::Brain>(m).is_some_and(|b| b.current_goal == Some(GoalKind::Raid)))
        .collect();
    subjects.sort_unstable();
    Some(MissionView { source, subjects })
}

/// Gangs with a raid worth a panel: a departure scheduled or recent, or a
/// stream still up, ascending id.
pub fn raids(world: &World) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world
        .gangs()
        .into_iter()
        .filter(|&g| {
            world.comp::<Gang>(g).is_some_and(|x| {
                x.raid_at.is_some()
                    || x.stream_by.is_some()
                    || x.last_raid_tick.is_some_and(|t| t + TICKS_PER_DAY > world.tick)
            })
        })
        .collect();
    v.sort_unstable();
    v
}

/// The raid's target building, whichever order aims it.
pub fn target(world: &World, gang: EntityId) -> Option<EntityId> {
    raid::gang_target(world, gang)
}

/// Where a subject stands: the building it is inside, else its tile.
pub fn whereabouts(world: &World, who: EntityId) -> String {
    match world.comp::<Position>(who) {
        Some(p) => match p.building {
            Some(b) => format!("inside {}", world.name_of(b)),
            None => format!("at ({}, {})", p.tile.x, p.tile.y),
        },
        None => "nowhere".to_string(),
    }
}
