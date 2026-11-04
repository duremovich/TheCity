//! Level of detail. Every 60 ticks the highest-priority agents become Full,
//! the next band Coarse and the rest Statistical; a calibrated hourly table
//! (`assets/stat_table.toml`, written by `citysim-cli calibrate`) stands in
//! for the brain of a Statistical agent. The tiers are kept as index lists on
//! the world (`World::tier`), and the Statistical hourly tick is spread over
//! the hour: an agent runs on the ticks where `id.index % 60 == tick % 60`
//! (`World::stat_slots` buckets the tier by that slot).
//!
//! M10: the table has 24 rows (phase x lawfulness x hunger) and five
//! independent hourly rolls beyond the one outcome: the agent's own theft and
//! courtship, and being robbed, beaten or killed by an actor nobody has drawn
//! yet. Those open a hole (`systems::bind`), bound later from the traces.

use rand::Rng;

use crate::components::{
    hole_id, trace_flags, Brain, Building, BuildingKind, Crime, DeathCause, Hole, HoleKind, Household, Job, Lod,
    MemoryKind, Personality, Position, Role, Sentence, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{self, ExecState};
use crate::goap::ActionKind;
use crate::systems::{bind, economy};
use crate::time::{DayPhase, Tick, TICKS_PER_HOUR};
use crate::world::{StatRow, World};

mod assign;
mod stat_needs;
mod stat_work;
mod statistical;

pub use assign::*;
pub use stat_needs::*;
pub use stat_work::*;
pub use statistical::*;

/// Tiles beyond the view rect that still count as on screen (and the camera ring's margin, `systems::ring`).
pub const VIEW_MARGIN: i32 = 8;
/// Ranks past a boundary within which an incumbent keeps its tier.
const HYSTERESIS_BAND: usize = 10;

/// Plan steps that keep an agent interesting while off screen.
pub fn story_relevant(kind: ActionKind) -> bool {
    matches!(
        kind,
        ActionKind::StealFood(_)
            | ActionKind::Arrest
            | ActionKind::Attack
            | ActionKind::Propose
            | ActionKind::JoinGang
            | ActionKind::Extort
            | ActionKind::Muster
            | ActionKind::Brawl
            | ActionKind::BuryCorpse
    )
}

pub fn run(world: &mut World) {
    if world.tick.is_multiple_of(TICKS_PER_HOUR) {
        // L2 (L23): yesterday's member-days into the ledger, before this
        // hour's assignment overwrites `body_day` (phase 4: the windows roll
        // for the victim side too).
        if world.tick_of_day() == 0 {
            crate::systems::fviolence::daily_actor(world);
        }
        // L2 phase 4: the day's order and vendetta sources, after the
        // midnight rescore.
        if world.tick_of_day() == TICKS_PER_HOUR as u16 {
            crate::systems::fviolence::rebuild_active(world);
        }
        crate::prof::timed!("lod::assign_hour", assign_hour(world));
        // L2 phase 4 (Ledger maths): the hour's body exposure.
        crate::systems::fviolence::tally_exposure(world);
        #[cfg(debug_assertions)]
        {
            let checked = world.check_indices();
            debug_assert!(checked.is_ok(), "{checked:?}");
        }
        // The trace's STATISTICAL_ALL_DAY: a body at any assignment today;
        // and (Shadow V4 review 3) its FULL: at Full at any.
        let today = world.day();
        for id in world.bodies() {
            let full = world.comp_mut::<Brain>(id).is_some_and(|b| {
                b.body_day = Some(today);
                b.lod == Lod::Full
            });
            if full {
                world.mark_day(id, crate::components::trace_flags::FULL);
            }
        }
    }
    // The view milestone V5 (OPEN-149): the camera ring's tiers, each tick, after the hour's assignment.
    crate::prof::timed!("lod::ring", crate::systems::ring::run(world));
    crate::prof::timed!("lod::run_statistical", run_statistical(world));
    // Promotions this tick queued their victims' open holes (M10 D32).
    bind::drain_queue(world);
}

fn assign_hour(world: &mut World) {
    if let Some(forced) = world.config.lod.force {
        // Prisoners and emigrants stay Coarse here too: a Statistical prisoner
        // would be snapped out of the Jail and never fed. A forced
        // Statistical city also keeps the bodies the default tiers always
        // give one (M10): guards and gravediggers (D20) and the wanted, so
        // its off-screen thieves can still be arrested.
        // scan-ok: hourly: forced tiers
        for id in world.citizens() {
            if !world.has::<Brain>(id) {
                continue;
            }
            let held = held_body(world, id);
            let body = forced == Lod::Statistical && stat_body(world, id);
            let lod = if crate::systems::abroad::is_away(world, id) {
                Lod::Statistical
            } else if held || body {
                Lod::Coarse
            } else {
                forced
            };
            set_lod(world, id, lod);
        }
    } else {
        assign(world);
    }
}

/// A prisoner, an emigrant or a cuffed agent: never Statistical in a
/// forced city (`assign_hour`).
fn held_body(world: &World, id: EntityId) -> bool {
    world.has::<Sentence>(id) || world.comp::<Brain>(id).is_some_and(|b| b.emigrating || b.cuffed_by.is_some())
}

/// The bodies a forced Statistical city keeps (`assign_hour`): a guard or
/// gravedigger and the wanted.
fn stat_body(world: &World, id: EntityId) -> bool {
    body_role(world, id) || crate::systems::law::wanted(world, id)
}

/// Lives at work gate (OPEN-106): an agent the Statistical tier never
/// holds, whatever the city's tiers: held, a guard or gravedigger, or
/// wanted (the forced Statistical city's Coarse bodies, `assign_hour`).
/// `calibrate` learns the Statistical evening's `p_venue` without them:
/// the evening pays the rate to the tier's adults while these bodies go out
/// on their own on screen, so a mean that held them counted their visits
/// twice off screen (the parity city's Bar 1.52 vs Full 0.82).
pub fn kept_body(world: &World, id: EntityId) -> bool {
    held_body(world, id) || stat_body(world, id)
}

/// A guard or gravedigger: a job the hourly table cannot do, so it always
/// keeps a body (M10 D20).
fn body_role(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| matches!(j.role, Role::Guard | Role::Gravedigger))
}

/// L2 (L21): sentenced agents are held Statistical.
pub fn held_on(world: &World) -> bool {
    world.config.lod.held_prisoners
}

/// L2 (L21): a held prisoner: sentenced and Statistical.
pub fn is_held(world: &World, id: EntityId) -> bool {
    world.has::<Sentence>(id) && world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical)
}

/// Round 6 V5 (TURF § 3): tile `t` lies in the camera's view (with the
/// margin): an off-screen corner fight's Statistical fighters there are
/// promoted (`turf::fighters`). No view (a headless run): nothing is.
pub fn on_screen(world: &World, t: TilePos) -> bool {
    let Some(r) = world.view_rect else { return false };
    let (x, y) = (i32::from(t.x), i32::from(t.y));
    x >= i32::from(r.x) - VIEW_MARGIN
        && x < i32::from(r.x) + i32::from(r.w) + VIEW_MARGIN
        && y >= i32::from(r.y) - VIEW_MARGIN
        && y < i32::from(r.y) + i32::from(r.h) + VIEW_MARGIN
}

/// L2 (L21, L22): a gang's raid window: a raid order whose muster is
/// within `raid_promote_hours` (or past), until the raid resolves and
/// clears `raid_at`.
pub fn raid_window(world: &World, gang: EntityId) -> bool {
    let hours = Tick::from(world.config.lod.raid_promote_hours) * TICKS_PER_HOUR;
    world
        .comp::<crate::components::Gang>(gang)
        .is_some_and(|g| g.order.is_raid() && g.raid_at.is_some_and(|t| world.tick + hours >= t))
}

/// L2 (L21): a sentenced agent who holds a body outside the rank: release
/// within `release_soon_hours` (the walk out is real), in cuffs, or of a
/// gang whose BreakOut musters within the raid window (the freed walk).
pub fn prisoner_body(world: &World, id: EntityId) -> bool {
    let Some(s) = world.comp::<Sentence>(id) else { return false };
    let soon = Tick::from(world.config.lod.release_soon_hours) * TICKS_PER_HOUR;
    if s.until_tick.saturating_sub(world.tick) <= soon {
        return true;
    }
    if world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_some()) {
        return true;
    }
    world.gang_of(id).is_some_and(|g| {
        world.comp::<crate::components::Gang>(g).is_some_and(|x| x.order == crate::components::Order::BreakOut)
            && raid_window(world, g)
    })
}

/// L2 (L22): the hour's class-2 sets: per gang its first `quota` members
/// (leader, the next two of `leader_ranking`, story steps, the front line,
/// then the daily rotation), and the first `private_quota` private guards
/// by id.
#[derive(Clone, Debug, Default)]
pub struct BudgetSets {
    pub gang: std::collections::BTreeSet<EntityId>,
    pub private: std::collections::BTreeSet<EntityId>,
    /// M16a (plan C35, phase 4): the parties of the records worked live,
    /// whole records by price inside `contract_quota` (class 4).
    pub contract: std::collections::BTreeSet<EntityId>,
    /// M16a (plan C35, phase 4): Fixer owners in the matching window, by
    /// book size inside `fixer_quota` (class 2).
    pub fixer: std::collections::BTreeSet<EntityId>,
    /// The island pass phase 8 (`[island_trades] cab_quota`; OPEN-168):
    /// each Garage's cab drivers on shift (or within the hour before it)
    /// who keep a body (class 2), lowest id first.
    pub cab: std::collections::BTreeSet<EntityId>,
}

/// The tile a gang's front line stands near: under Contest the rival's
/// held Home door nearest the Hideout; under a raid order the target's
/// door; under Harvest the target. `None` otherwise.
fn front_tile(world: &World, gang: EntityId) -> Option<TilePos> {
    use crate::components::{Gang, Order};
    let g = world.comp::<Gang>(gang)?;
    let hideout = world.comp::<Building>(g.hideout).map(|b| b.door)?;
    match g.order {
        Order::Contest => {
            let rival = world.rival_of(gang)?;
            world
                .comp::<Gang>(rival)?
                .territory
                .iter()
                .filter_map(|&h| world.comp::<Building>(h).map(|b| (b.door.manhattan(hideout), h.index, b.door)))
                .min()
                .map(|(_, _, t)| t)
        }
        o if o.is_raid() => {
            crate::systems::raid::gang_target(world, gang).and_then(|b| world.comp::<Building>(b)).map(|b| b.door)
        }
        Order::Harvest => g.harvest_target.and_then(|t| world.comp::<Position>(t)).map(|p| p.tile),
        _ => None,
    }
}

/// L2 (L22): the members of one gang holding class 2 this hour, in rank
/// order: the leader, the next two of `leader_ranking`, members with a
/// `story_relevant` step, members within `front_tiles` of the order's
/// target (nearest first), then the daily rotation
/// (`splitmix64(seed ^ id ^ day)` ascending); `raid_quota` of them in a
/// raid window, else `gang_quota`.
pub fn gang_quota_members(world: &World, gang: EntityId) -> Vec<EntityId> {
    use crate::components::Gang;
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    let quota = if raid_window(world, gang) { world.config.lod.raid_quota } else { world.config.lod.gang_quota };
    let free: Vec<EntityId> =
        g.members.iter().copied().filter(|&m| world.has::<Brain>(m) && !world.has::<Sentence>(m)).collect();
    let mut out: Vec<EntityId> = Vec::with_capacity(quota.min(free.len()));
    let push = |out: &mut Vec<EntityId>, m: EntityId| {
        if out.len() < quota && !out.contains(&m) {
            out.push(m);
        }
    };
    if let Some(l) = g.leader.filter(|l| free.contains(l)) {
        push(&mut out, l);
    }
    for m in crate::systems::gang::leader_ranking(world, gang).into_iter().filter(|&m| Some(m) != g.leader).take(2) {
        push(&mut out, m);
    }
    for &m in &free {
        if world.comp::<Brain>(m).and_then(|b| b.current_step()).is_some_and(|s| story_relevant(s.action)) {
            push(&mut out, m);
        }
    }
    if let Some(front) = front_tile(world, gang) {
        let reach = world.config.lod.front_tiles;
        let mut near: Vec<(u32, EntityId)> = free
            .iter()
            .filter_map(|&m| world.comp::<Position>(m).map(|p| (p.tile.manhattan(front), m)))
            .filter(|&(d, _)| d <= reach)
            .collect();
        near.sort_unstable();
        for (_, m) in near {
            push(&mut out, m);
        }
    }
    let seed = world.seed();
    let day = world.day();
    let mut rest: Vec<(u64, EntityId)> =
        free.iter().map(|&m| (crate::rng::splitmix64(seed ^ u64::from(m.index) ^ day.rotate_left(32)), m)).collect();
    rest.sort_unstable();
    for (_, m) in rest {
        push(&mut out, m);
    }
    out
}

/// L2 (L22): the hour's class-2 sets (`assign` computes them once).
pub fn budget_sets(world: &World) -> BudgetSets {
    let mut sets = BudgetSets::default();
    for gang in world.gangs() {
        sets.gang.extend(gang_quota_members(world, gang));
    }
    let quota = world.config.lod.private_quota;
    sets.private.extend(
        world.guards().iter().copied().filter(|&g| crate::systems::law::is_private_guard(world, g)).take(quota),
    );
    // M16a (plan C35, phase 4).
    sets.contract = crate::systems::contracts::contract_set(world);
    sets.fixer = crate::systems::contracts::fixer_set(world);
    sets.cab = crate::systems::streetwork::cab_body_set(world).into_iter().collect();
    sets
}

/// L2 (L22): the public watch ranks 3 only on shift (or within the hour
/// before it, so the commute is a body's), or while making an arrest.
fn watch_on_duty(world: &World, id: EntityId) -> bool {
    let tod = world.tick_of_day();
    let lead = ((u64::from(tod) + TICKS_PER_HOUR) % crate::time::TICKS_PER_DAY) as u16;
    world.comp::<Job>(id).is_some_and(|j| j.on_shift(tod) || j.on_shift(lead))
        || world.comp::<Brain>(id).is_some_and(|b| {
            b.escorting.is_some()
                || b.current_step().is_some_and(|s| matches!(s.action, ActionKind::Arrest | ActionKind::Escort))
        })
}
