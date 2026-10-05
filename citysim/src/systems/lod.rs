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
use crate::time::{DayPhase, TICKS_PER_HOUR};
use crate::world::{StatRow, World};

/// Tiles beyond the view rect that still count as on screen.
const VIEW_MARGIN: i32 = 8;
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
        assign_hour(world);
        #[cfg(debug_assertions)]
        {
            let checked = world.check_indices();
            debug_assert!(checked.is_ok(), "{checked:?}");
        }
        // The trace's STATISTICAL_ALL_DAY: a body at any assignment today.
        let today = world.day();
        for id in world.bodies() {
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.body_day = Some(today);
            }
        }
    }
    run_statistical(world);
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
            let Some(b) = world.comp::<Brain>(id) else { continue };
            let held = world.has::<Sentence>(id) || b.emigrating || b.cuffed_by.is_some();
            let body = forced == Lod::Statistical && (body_role(world, id) || crate::systems::law::wanted(world, id));
            set_lod(world, id, if held || body { Lod::Coarse } else { forced });
        }
    } else {
        assign(world);
    }
}

/// A guard or gravedigger: a job the hourly table cannot do, so it always
/// keeps a body (M10 D20).
fn body_role(world: &World, id: EntityId) -> bool {
    world.comp::<Job>(id).is_some_and(|j| matches!(j.role, Role::Guard | Role::Gravedigger))
}

/// Rank every living adult and hand out the tiers with hysteresis. Jailed
/// and emigrating agents are Coarse without taking a slot.
fn assign(world: &mut World) {
    let view = world.view_rect;
    let centre = match view {
        Some(r) => TilePos { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        None => TilePos { x: (world.map.w() / 2) as u8, y: (world.map.h() / 2) as u8 },
    };
    let on_screen = |t: TilePos| -> bool {
        let Some(r) = view else { return false };
        let (x, y) = (i32::from(t.x), i32::from(t.y));
        x >= i32::from(r.x) - VIEW_MARGIN
            && x < i32::from(r.x) + i32::from(r.w) + VIEW_MARGIN
            && y >= i32::from(r.y) - VIEW_MARGIN
            && y < i32::from(r.y) + i32::from(r.h) + VIEW_MARGIN
    };

    let mut ranked: Vec<(i32, u32, u32, EntityId)> = Vec::new();
    // scan-ok: hourly: assign
    for id in world.citizens() {
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        if world.has::<Sentence>(id) || brain.emigrating {
            set_lod(world, id, Lod::Coarse);
            continue;
        }
        // A wanted agent counts as a story step: a reported thief must stay on
        // the map for the arrest path to reach them.
        let story =
            brain.current_step().is_some_and(|s| story_relevant(s.action)) || crate::systems::law::wanted(world, id);
        // Gang members are never Statistical: the hourly table has no orders,
        // claims or raids, and two gangs fit inside the Coarse budget.
        let gang = world.has::<crate::components::GangMember>(id);
        // M10 D20: guards rank with gang members, or at 2,000 the gangs take
        // every Coarse slot and the whole watch is Statistical. Gravediggers
        // too: the hourly table cannot bury, and a Statistical digger never
        // even learns of a corpse (it keeps no SawCorpse memory).
        let guard = body_role(world, id);
        // M11: a private guard keeps a body but ranks with the gangs: at the
        // watch's rank the twelve took Full slots and walked their corp's
        // Blocks tile by tile (-800 ticks/s at 2,000).
        let private = guard && crate::systems::law::is_private_guard(world, id);
        // Pinned tops the ladder, then the watch (3), then gang members (2):
        // at the Coarse cap the farthest gang member falls first, not a guard.
        let class = if brain.pinned {
            5
        } else if guard && !private {
            3
        } else if gang || private {
            2
        } else {
            0
        };
        // M11: a fresh lawless evictee (D36: so the spiral's first link,
        // JoinGang, can be planned) and an agent able to found (D25) get a
        // body; neither can happen in the hourly table.
        let m11 =
            crate::systems::classes::evicted_desperate(world, id) || crate::systems::founding::can_found(world, id);
        let priority = i32::from(on_screen(pos.tile)) * 4 + class + i32::from(story || m11);
        ranked.push((-priority, pos.tile.manhattan(centre), id.index, id));
    }
    ranked.sort_unstable();
    let ids: Vec<EntityId> = ranked.iter().map(|&(_, _, _, id)| id).collect();
    let prio: Vec<i32> = ranked.iter().map(|&(p, _, _, _)| -p).collect();
    let current: Vec<Lod> = ids.iter().map(|&id| world.comp::<Brain>(id).map_or(Lod::Coarse, |b| b.lod)).collect();

    let max_full = world.config.lod.max_full;
    let max_coarse = world.config.lod.max_coarse;
    let mut tier: Vec<Lod> = (0..ids.len())
        .map(|rank| {
            if rank < max_full {
                Lod::Full
            } else if rank < max_full + max_coarse {
                Lod::Coarse
            } else {
                Lod::Statistical
            }
        })
        .collect();
    // Hysteresis at each boundary: an incumbent ranked just past it keeps its
    // tier against a newcomer of equal priority (interchangeable agents do
    // not swap tiers over a tile of distance); a pinned or on-screen or
    // story newcomer always takes the slot. Newcomers at the Coarse line are
    // the Statistical: a Full agent being demoted keeps its plan as Coarse.
    for (upper, boundary) in [(Lod::Full, max_full), (Lod::Coarse, max_full + max_coarse)] {
        let lower = if upper == Lod::Full { Lod::Coarse } else { Lod::Statistical };
        let is_newcomer =
            |r: usize| if upper == Lod::Full { current[r] != Lod::Full } else { current[r] == Lod::Statistical };
        let band_end = (boundary + HYSTERESIS_BAND).min(ids.len());
        for rank in boundary..band_end {
            if current[rank] != upper || tier[rank] == upper {
                continue;
            }
            let newcomer = (0..boundary).rev().find(|&r| tier[r] == upper && is_newcomer(r) && prio[r] == prio[rank]);
            if let Some(r) = newcomer {
                tier[r] = lower;
                tier[rank] = upper;
            }
        }
    }
    for (i, &id) in ids.iter().enumerate() {
        set_lod(world, id, tier[i]);
    }
}

/// Change an agent's LOD, converting its execution state per the transition
/// table: a walk becomes a timed arrival and back; Use and Wait keep their
/// remaining duration; a demotion to Statistical abandons the plan and snaps
/// the agent to the door its day phase implies; a promotion places it there.
pub fn set_lod(world: &mut World, id: EntityId, lod: Lod) {
    let Some(brain) = world.comp::<Brain>(id) else { return };
    let from = brain.lod;
    if from == lod {
        return;
    }
    if lod == Lod::Statistical {
        if brain.plan.is_some() {
            let goal = brain.plan_goal();
            world.abort_plan(id);
            world.push_event(EventKind::PlanAborted, &[id], format!("{goal:?} LodDemotion"));
        }
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.lod = Lod::Statistical;
            b.exec = ExecState::Idle;
            b.current_goal = None;
            b.plan_queued = false;
        }
        world.retier(id);
        world.plan_queue.retain(|&(_, who), _| who != id);
        snap_to_phase_door(world, id, false);
        return;
    }
    if from == Lod::Statistical {
        let today = world.day();
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.lod = lod;
            b.exec = ExecState::Idle;
            b.current_goal = None;
            b.body_day = Some(today);
        }
        world.retier(id);
        snap_to_phase_door(world, id, true);
        // A body's memories are never half-filled: its open victim holes bind
        // at the end of this tick's `lod::run`.
        if let Some(hs) = world.holes_by_agent.get(&id) {
            let hs = hs.clone();
            world.bind_queue.extend(hs);
        }
        return;
    }
    let exec = brain.exec.clone();
    let new_exec = match (exec, lod) {
        (ExecState::Goto { target, .. }, Lod::Coarse) => exec::timed_goto(world, id, target),
        (ExecState::GotoTimed { target, .. }, Lod::Full) => match exec::walking_goto(world, id, target) {
            Some(state) => state,
            // Unreachable on foot: let the step fail and the agent replan.
            None => ExecState::Idle,
        },
        (other, _) => other,
    };
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.lod = lod;
        b.exec = new_exec;
    }
    world.retier(id);
}

/// The door an off-screen agent stands at for the current phase. Demotion:
/// Home at Night/Morning, workplace in the Work phase if employed, else
/// Home; Market if homeless. Promotion adds the Evening venue: the Bar for
/// the sociable, else the Market.
fn snap_to_phase_door(world: &mut World, id: EntityId, promotion: bool) {
    let phase = world.phase();
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let workplace = world.comp::<Job>(id).and_then(|j| j.employer);
    let sociable = world.comp::<Personality>(id).is_some_and(|p| p.sociability >= 0.5);
    let building = match phase {
        DayPhase::Night | DayPhase::Morning => home,
        DayPhase::Work => workplace.or(home),
        DayPhase::Evening if promotion => {
            if sociable {
                world.local(id, BuildingKind::Bar)
            } else {
                world.local(id, BuildingKind::Market)
            }
        }
        DayPhase::Evening => home,
    }
    .or_else(|| world.local(id, BuildingKind::Market));
    if let Some(b) = building {
        world.stand_at_door(id, b);
    }
}

/// The agent's table row: the current phase, and its lawfulness and hunger
/// at the start of the hour (before the hour's decay), as `calibrate` buckets.
fn stat_row(world: &World, id: EntityId) -> Option<StatRow> {
    let t = world.stat_table.as_ref()?;
    let lawfulness = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
    Some(t.row(world.phase(), lawfulness, hunger).clone())
}

/// Statistical agents whose hourly slot is this tick: `id.index % 60 == tick % 60`.
/// Each agent keeps a fixed minute of the hour, so it runs 24 times a day and
/// the load is spread evenly over the ticks.
/// Reads one slot bucket (`World::stat_slots`), so the cost is O(due agents).
pub fn due_this_tick(world: &World) -> Vec<EntityId> {
    world.stat_slots.at(world.tick).iter().copied().filter(|&id| !world.has::<Sentence>(id)).collect()
}

/// The hourly stand-in for a Statistical brain: an hour of need decay, then
/// one outcome drawn from the calibrated row with the agent's own stream. It
/// runs every tick for the agents due this tick (`due_this_tick`).
pub fn run_statistical(world: &mut World) {
    let agents = due_this_tick(world);
    if agents.is_empty() {
        return;
    }
    if world.stat_table.is_none() {
        if world.stat_table_legacy {
            panic!(
                "assets/stat_table.toml is the pre-M10 four-row table: run `cargo run --release -p citysim-cli -- calibrate`"
            );
        }
        panic!("assets/stat_table.toml is missing: run `cargo run --release -p citysim-cli -- calibrate`");
    }
    let cfg = world.config.needs.clone();
    let season = world.season().index();
    let season_energy_mult = world.config.economy.energy_decay_mult[season];
    let phase = world.phase();
    for id in agents {
        let Some(row) = stat_row(world, id) else { continue };
        // 1. An hour of decay in one step.
        let sociability = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
        let under_18 = !crate::systems::demography::is_adult(world, id);
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            let ctx =
                crate::needs::DecayCtx { season_energy_mult, sociability, under_18, ..crate::needs::DecayCtx::plain() };
            crate::needs::decay(n, &cfg, &ctx, TICKS_PER_HOUR as u32);
        }
        crate::needs::starvation(world, id);
        if !world.has::<Brain>(id) {
            continue; // starved this hour
        }
        // 2b. Work is not a gamble: an employed agent on shift works the hour
        // (a Full farmer works every hour of the shift), and the jobless draw
        // the dole once a day. The table's p_work mass then stands for the
        // idle hours of the employed.
        stat_work(world, id, phase);
        // 3. One outcome. A Full agent eats when hungry, not by lottery, so a
        // hungry Statistical agent eats if it can and the draw covers the
        // discretionary meals of the fed.
        // ... and a sated one does not (a Full agent skips Eat above ~0.7).
        let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
        let (hungry, sated) = (hunger < 0.4, hunger >= 0.7);
        let u: f32 = world.rng.agent(id).random();
        let outcome = if hungry || (u < row.p_eat && !sated) {
            Outcome::Eat
        } else if u < row.p_eat + row.p_work {
            Outcome::Idle
        } else if u < row.p_eat + row.p_work + row.p_social {
            Outcome::Social
        } else if u < row.p_eat + row.p_work + row.p_social + row.p_sleep {
            Outcome::Sleep
        } else {
            Outcome::Idle
        };
        match outcome {
            Outcome::Eat => stat_eat(world, id),
            Outcome::Social => stat_social(world, id),
            Outcome::Sleep => {
                if phase == DayPhase::Night {
                    // As a Full agent's Sleep at home: energy back, and a
                    // cohabiting spouse keeps the marriage warm.
                    let home = world.comp::<Household>(id).and_then(|h| h.home);
                    if home.is_some() {
                        world.mark_day(id, trace_flags::SLEPT_AT_HOME);
                    }
                    let with_spouse = home.is_some()
                        && world.spouse_of(id).is_some_and(|s| world.comp::<Household>(s).and_then(|h| h.home) == home);
                    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                        n.energy = 1.0;
                        if with_spouse {
                            n.intimacy = (n.intimacy + 0.4).min(1.0);
                        }
                    }
                }
            }
            Outcome::Idle => {}
        }
        // 4. The independent rolls: the agent's own crimes and courtship, and
        // what is done to it by an actor nobody has drawn yet (a hole).
        stat_rolls(world, id, &row);
        if world.has::<Brain>(id) {
            world.recompute_wealth_for(id);
        }
    }
}

/// A first meeting off screen (the `p_meet` roll): a stranger from the
/// agent's zone, at the v1 first-meeting affinity. No Chat's drift: a Full
/// first meeting (`social::colocation`) is the bare edge, Chats are the
/// separately calibrated `p_chat` roll, and drifting every new acquaintance
/// past the prune line kept them all (the 2,000 save grew 30 -> 48 MB).
fn stat_meet(world: &mut World, id: EntityId) {
    let Some(stranger) = stranger_in_zone(world, id) else { return };
    let sa = world.comp::<Personality>(id).map_or(0.5, |p| p.sociability);
    let sb = world.comp::<Personality>(stranger).map_or(0.5, |p| p.sociability);
    let rng = world.rng.agent(id);
    let (u1, u2): (f32, f32) = (rng.random(), rng.random());
    let affinity = crate::systems::social::first_affinity(sa, sb, u1, u2);
    crate::systems::social::first_meeting(world, id, stranger, affinity);
}

/// A stranger for an off-screen meeting: up to `STRANGER_TRIES` uniform
/// draws from the Statistical tier on the agent's stream, the first standing
/// in the agent's zone taken (rejection sampling: uniform over that zone's
/// Statistical agents). `None` when no draw lands in the zone, or the one
/// that does is a child, the agent itself or someone it already knows: no
/// meeting this hour. Only Statistical strangers: bodies meet by co-location,
/// and drawing them gave the gang members and guards hundreds of
/// acquaintances that every think walks. A scan of the zone's Homes per
/// meeting cost 7 % of the 2,000 city's throughput.
fn stranger_in_zone(world: &mut World, id: EntityId) -> Option<EntityId> {
    const STRANGER_TRIES: usize = 16;
    let zone_of = |w: &World, o: EntityId| w.comp::<Position>(o).map(|p| w.map.zone(p.tile));
    let zone = zone_of(world, id)?;
    let n = world.tier(Lod::Statistical).len();
    if n == 0 {
        return None;
    }
    for _ in 0..STRANGER_TRIES {
        let k = world.rng.agent(id).random_range(0..n);
        let o = world.tier(Lod::Statistical)[k];
        if zone_of(world, o) != Some(zone) {
            continue;
        }
        let ok = o != id && crate::systems::demography::is_adult(world, o) && world.edge(id, o).is_none();
        return ok.then_some(o);
    }
    None
}

/// The seven independent hourly rolls, always drawn from the agent's stream in
/// the fixed order steal, flirt, robbed, assaulted, killed, meet, chat (so the stream
/// advances identically whatever fires), then applied killed first. An agent
/// whose hour already took it off the tier (a caught thief) only draws.
fn stat_rolls(world: &mut World, id: EntityId, row: &StatRow) {
    let rng = world.rng.agent(id);
    let u_steal: f32 = rng.random();
    let u_flirt: f32 = rng.random();
    let u_robbed: f32 = rng.random();
    let u_assaulted: f32 = rng.random();
    let u_killed: f32 = rng.random();
    let u_meet: f32 = rng.random();
    let u_chat: f32 = rng.random();
    if world.comp::<Brain>(id).is_none_or(|b| b.lod != Lod::Statistical) {
        return;
    }
    let m = world.config.lod.stat_violence_mult;
    let adult = crate::systems::demography::is_adult(world, id);
    let Some(tile) = world.comp::<Position>(id).map(|p| p.tile) else { return };
    let zone = world.map.zone(tile);
    let tick = world.tick;
    let name = world.name_of(id);
    let consequential = crate::systems::law::is_guard(world, id) || world.has::<crate::components::GangMember>(id);
    let base = |kind: HoleKind, event_id: u64, consequential: bool, loot: i64, w: &World| Hole {
        id: hole_id(tick, id, kind),
        kind,
        victim: id,
        zone,
        tick,
        event_id,
        consequential,
        spouse: w.spouse_of(id),
        loot,
        home: w.comp::<Household>(id).and_then(|h| h.home),
        gang: w.gang_of(id),
    };

    if adult && u_killed < row.p_killed * m {
        let hole = base(HoleKind::Killed, 0, true, 0, world);
        let ev = world.push_event(
            EventKind::Murder,
            &[EntityId::NONE, id],
            format!("{name} was killed in the {zone} (assailant unknown)"),
        );
        world.kill_by(id, DeathCause::Violence, None);
        world.stats.current.deaths_violence_offscreen += 1;
        bind::open_hole(world, Hole { event_id: ev, ..hole });
        return;
    }
    if u_assaulted < row.p_assaulted * m {
        world.remember(id, MemoryKind::Fought, None, 0.7, -0.7, false);
        world.remember(id, MemoryKind::Lost, None, 0.6, -0.6, false);
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            n.safety = (n.safety - 0.6).max(0.0);
            n.energy = (n.energy - 0.2).max(0.0);
        }
        // As `law::raise_crime` drifts a Full victim.
        if let Some(p) = world.comp_mut::<Personality>(id) {
            p.drift(crate::personality::Drift::Robbed);
        }
        let ev =
            world.push_event(EventKind::Assaulted, &[EntityId::NONE, id], format!("{name} was beaten in the {zone}"));
        let hole = base(HoleKind::Assaulted, ev, consequential, 0, world);
        bind::open_hole(world, hole);
    }
    if u_robbed < row.p_robbed * m {
        let extort = world.config.social.extort_amount;
        let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
        let loot = extort.min(coins.max(0));
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(id) {
            w.coins -= loot;
        }
        world.remember(id, MemoryKind::WasRobbed, None, 0.6, -0.6, false);
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            n.safety = (n.safety - 0.4).max(0.0);
        }
        if let Some(p) = world.comp_mut::<Personality>(id) {
            p.drift(crate::personality::Drift::Robbed);
        }
        let ev = world.push_event(
            EventKind::Robbed,
            &[EntityId::NONE, id],
            format!("{name} was robbed of {loot} in the {zone}"),
        );
        let hole = base(HoleKind::Robbed, ev, consequential, loot, world);
        bind::open_hole(world, hole);
    }
    // Meet before courting: Full agents court the people they meet.
    if u_meet < row.p_meet {
        stat_meet(world, id);
    }
    if u_chat < row.p_chat {
        stat_chat(world, id, row);
    }
    if u_flirt < row.p_flirt {
        stat_flirt(world, id);
    }
    if u_steal < row.p_steal {
        stat_theft(world, id);
    }
}

/// A Flirt off screen, as `exec/actions.rs` Flirt: the highest-affinity known
/// edge (>= 0.3) with a living, adult, unmarried counterpart not rejected this
/// week (ties: lower id). Ready to propose: the proposal (the second visit of
/// the v1 courtship). Else accepted at the v1 rate `0.3 + affinity + 0.2 x
/// sociability`, rolled on the agent's stream.
fn stat_flirt(world: &mut World, id: EntityId) {
    use crate::systems::{demography, social};
    if !demography::is_adult(world, id) || social::has_spouse(world, id) {
        return;
    }
    // The same candidate rule `calibrate` conditions `p_flirt` on.
    let Some(partner) = social::known_candidate(world, id, 0.3).filter(|&o| demography::is_adult(world, o)) else {
        return;
    };
    if social::propose_allowed(world, id, partner) {
        social::propose(world, id, partner);
        return;
    }
    let aff = world.edge(id, partner).map_or(0.0, |e| e.affinity);
    let sociable = world.comp::<Personality>(partner).map_or(0.5, |p| p.sociability);
    let roll: f32 = world.rng.agent(id).random();
    if roll < 0.3 + aff + 0.2 * sociable {
        for who in [id, partner] {
            if let Some(n) = world.comp_mut::<crate::components::Needs>(who) {
                n.intimacy = (n.intimacy + 0.1).min(1.0);
            }
            world.remember(who, MemoryKind::Courted, Some(if who == id { partner } else { id }), 0.4, 0.3, false);
        }
        social::adjust(world, id, partner, 0.1, 0.02);
    } else {
        world.remember(id, MemoryKind::Rejected, Some(partner), 0.4, -0.3, false);
    }
}

/// An off-screen Market theft: one unit from the agent's Market into its
/// inventory as stolen food, the `Theft` event and counter, and a
/// `stat_theft_caught_p` report with no witness (the thief then gets a body,
/// which the arrest path needs). Shared by the eat path and the `p_steal`
/// roll. Returns whether anything was taken.
fn stat_theft(world: &mut World, id: EntityId) -> bool {
    let Some(market) = world.local(id, BuildingKind::Market) else { return false };
    let Some(b) = world.comp_mut::<Building>(market).filter(|b| b.stock_food > 0) else { return false };
    b.stock_food -= 1;
    // M11 D20: a corp-owned Market books the unit at its price.
    let coins = world.price_at(market);
    crate::systems::ownership::note_loss(world, market, coins, Some(id));
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
        i.food += 1;
        i.stolen_food += 1;
    }
    world.stats.current.thefts += 1;
    let name = world.name_of(id);
    world.push_event(EventKind::Theft, &[id, market], format!("{name} stole food (off screen)"));
    let caught: f64 = world.rng.agent(id).random();
    let p = world
        .stat_table
        .as_ref()
        .and_then(|t| t.p_theft_caught)
        .map_or(world.config.crime.stat_theft_caught_p, f64::from);
    if caught < p {
        crate::systems::law::file_report(world, Crime::Theft, id, None);
        // The normal arrest path needs a body on the map.
        set_lod(world, id, Lod::Coarse);
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Eat,
    Social,
    Sleep,
    Idle,
}

fn stat_eat(world: &mut World, id: EntityId) {
    let cfg = world.config.needs.clone();
    let has_food = world.comp::<crate::components::Inventory>(id).is_some_and(|i| i.food >= 1);
    if has_food {
        if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
            i.food -= 1;
            i.stolen_food = i.stolen_food.min(i.food);
        }
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            crate::needs::eat(n, &cfg);
        }
        world.mark_day(id, trace_flags::ATE);
        return;
    }
    // The Home pantry feeds a Statistical resident as it feeds a Full one.
    let pantry = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.stock_food > 0));
    if let Some(h) = pantry {
        if let Some(b) = world.comp_mut::<Building>(h) {
            b.stock_food -= 1;
        }
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            crate::needs::eat(n, &cfg);
        }
        world.mark_day(id, trace_flags::ATE);
        return;
    }
    let Some((market, stock, price)) = world
        .local(id, BuildingKind::Market)
        .and_then(|m| world.comp::<Building>(m).map(|b| (m, b.stock_food, world.price_at(m))))
    else {
        return;
    };
    if stock == 0 {
        return;
    }
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    if coins >= price {
        // The purchase a Full agent makes (BuyFood: up to three units), one
        // eaten and the rest stored in the Home pantry (StoreFood), where the
        // household eats it. Buying one meal at a time left a broke
        // housemate nothing to eat once the opening pantry was gone (M10).
        let units = economy::buy_quantity(world, id, Some(market)).max(1);
        let paid = economy::pay_for_food(world, id, Some(market), units);
        if economy::take_food(world, id, Some(market), units, paid) {
            if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
                i.food = i.food.saturating_sub(1);
            }
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
            world.mark_day(id, trace_flags::ATE);
            stat_store_food(world, id);
        }
        return;
    }
    // Desperation theft: a starving agent with no food and no coins steals,
    // whatever its lawfulness (with nothing else to eat that is a Full
    // agent's only Eat plan). Every other theft is the table's `p_steal`,
    // calibrated on all Full thefts but these (M10; replaces the v1
    // lawless-and-hungry gate, D29).
    let starving = world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger <= 0.0);
    if !starving {
        return;
    }
    if !stat_theft(world, id) {
        return;
    }
    // ... and eaten at once.
    if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
        i.food = i.food.saturating_sub(1);
        i.stolen_food = i.stolen_food.min(i.food);
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    world.mark_day(id, trace_flags::ATE);
}

/// StoreFood off screen: the agent's bought (not stolen) food goes into its
/// Home pantry up to the pantry's cap.
fn stat_store_food(world: &mut World, id: EntityId) {
    let Some(home) = world.comp::<Household>(id).and_then(|h| h.home) else { return };
    let cap = world.config.buildings.home.stock_cap;
    let room = world.comp::<Building>(home).map_or(0, |b| cap.saturating_sub(b.stock_food));
    let Some(inv) = world.comp_mut::<crate::components::Inventory>(id) else { return };
    let movable = inv.food.saturating_sub(inv.stolen_food).min(room);
    inv.food -= movable;
    if let Some(b) = world.comp_mut::<Building>(home) {
        b.stock_food += movable;
    }
}

fn stat_work(world: &mut World, id: EntityId, phase: DayPhase) {
    let tick = world.tick;
    let Some(job) = world.comp::<Job>(id).cloned() else {
        // The dole, paid directly, decided once a day in the Work phase as a
        // Full agent decides it (M10; drawing it daily regardless drained the
        // Treasury and left the wages unpaid): its Earn goal is satisfied
        // while it holds `SAVINGS_DAYS` meals of coins, and below that it
        // makes the walk to the Hall on the table's `p_dole_day` of days.
        // An agent who cannot buy today's meal always goes (M10 phase 5b): a
        // broke Full agent collects on ~97 % of days, and each day a broke
        // Statistical agent lost the lottery became a desperation theft.
        let day = world.day();
        let price = world.local(id, BuildingKind::Market).map_or(1, |m| world.price_at(m));
        let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
        let saving = coins >= crate::goap::world_state::SAVINGS_DAYS.saturating_mul(price);
        let undecided = world.comp::<Brain>(id).is_some_and(|b| b.last_dole_day != Some(day));
        if phase == DayPhase::Work && !saving && undecided && crate::systems::demography::is_adult(world, id) {
            let p = world.stat_table.as_ref().map_or(1.0, |t| t.p_dole_day);
            let u: f32 = world.rng.agent(id).random();
            if u < p || coins < price {
                economy::collect_dole(world, id);
            }
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.last_dole_day = Some(day);
            }
        }
        return;
    };
    let key = job.shift_key_at(tick);
    if !job.on_shift(world.tick_of_day()) || !crate::exec::routine::is_workday(key) {
        return;
    }
    // This shift is already done, or struck (M11 D35): no work, no wage.
    if job.last_shift_day == Some(key) {
        return;
    }
    if job.role == Role::Farmer {
        if let Some(farm) = job.employer {
            economy::accrue_farm_work(world, id, farm, u64::from(TICKS_PER_HOUR as u32));
        }
    }
    // An hour of co-work drifts each pair of co-workers as `social`'s
    // co-location does for bodies (bodies never see a Statistical agent in
    // their building). Each Statistical pair once an hour: the lower id drives.
    if job.employer.is_some() {
        let tod = world.tick_of_day();
        let mates: Vec<EntityId> = world
            .workers(job.role)
            .iter()
            .copied()
            .filter(|&c| c > id)
            .filter(|&c| {
                world.comp::<Job>(c).is_some_and(|j| j.employer == job.employer && j.on_shift(tod))
                    && world.comp::<Brain>(c).is_some_and(|b| b.lod == Lod::Statistical)
                    && !world.has::<Sentence>(c)
            })
            .collect();
        for c in mates {
            crate::systems::social::interacted(world, id, c);
        }
    }
    let shift_ends_soon = job.shift_end(tick).is_some_and(|end| end <= tick + TICKS_PER_HOUR);
    if shift_ends_soon && job.last_shift_day != Some(key) {
        if let Some(j) = world.comp_mut::<Job>(id) {
            j.last_shift_day = Some(key);
            j.days_unpaid = j.days_unpaid.saturating_add(1);
        }
        economy::collect_wage(world, id);
        // A Full farmer hauls to the Market at shift end; so does this one.
        if job.role == Role::Farmer {
            if let Some(farm) = job.employer {
                let enough =
                    world.comp::<Building>(farm).is_some_and(|b| b.stock_food >= world.config.economy.haul_min_stock);
                if enough {
                    economy::haul(world, farm);
                }
            }
        }
    }
}

/// An off-screen Social hour: belonging. The Chat it used to carry is the
/// `p_chat` roll.
fn stat_social(world: &mut World, id: EntityId) {
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.belonging = (n.belonging + 0.1).min(1.0);
    }
}

/// A Chat off screen (the `p_chat` roll): on the row's `p_chat_home`
/// (always drawn) with the housemate a Full agent at home would pick (the
/// highest affinity, ties to the lower id, as `best_colocated_partner`),
/// else with a random known edge, else (nobody known yet) a Statistical
/// housemate met.
fn stat_chat(world: &mut World, id: EntityId, row: &StatRow) {
    let u_home: f32 = world.rng.agent(id).random();
    if u_home < row.p_chat_home {
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let best = home
            .map(|h| world.residents_of(h))
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|&o| o != id && world.has::<Brain>(o) && !world.has::<Sentence>(o))
            .map(|o| (o, world.edge(id, o).map_or(0.0, |e| e.affinity)))
            .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
            .map(|(o, _)| o);
        if let Some(mate) = best {
            crate::systems::social::interacted(world, id, mate);
            return;
        }
    }
    let neighbours: Vec<EntityId> = world.neighbours(id).collect();
    if neighbours.is_empty() {
        // Nobody known yet: meet a Statistical housemate.
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let mates: Vec<EntityId> = home
            .map(|h| world.residents_of(h))
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|&o| o != id && world.comp::<Brain>(o).is_some_and(|b| b.lod == Lod::Statistical))
            .collect();
        if !mates.is_empty() {
            let k = world.rng.agent(id).random_range(0..mates.len());
            let tick = world.tick;
            let e = world.edge_entry(id, mates[k]);
            e.last_interaction = tick;
        }
        return;
    }
    // A Chat's drift (`exec/actions.rs` Chat): affinity by similarity, trust +0.02.
    let k = world.rng.agent(id).random_range(0..neighbours.len());
    crate::systems::social::interacted(world, id, neighbours[k]);
}
