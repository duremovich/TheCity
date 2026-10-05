//! Level of detail. Every 60 ticks the highest-priority agents become Full,
//! the next band Coarse and the rest Statistical; a calibrated hourly table
//! (`assets/stat_table.toml`, written by `citysim-cli calibrate`) stands in
//! for the brain of a Statistical agent. The tiers are kept as index lists on
//! the world (`World::tier`), and the Statistical hourly tick is spread over
//! the hour: an agent runs on the ticks where `id.index % 60 == tick % 60`.

use rand::Rng;

use crate::components::{
    Brain, Building, BuildingKind, Crime, Household, Job, Lod, Personality, Position, Role, Sentence, TilePos,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::exec::{self, ExecState};
use crate::goap::ActionKind;
use crate::map::{MAP_H, MAP_W};
use crate::systems::economy;
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
        debug_assert!(world.check_indices().is_ok(), "{:?}", world.check_indices());
    }
    run_statistical(world);
}

fn assign_hour(world: &mut World) {
    if let Some(forced) = world.config.lod.force {
        // Prisoners and emigrants stay Coarse here too: a Statistical prisoner
        // would be snapped out of the Jail and never fed.
        // scan-ok: hourly: forced tiers
        for id in world.citizens() {
            let Some(b) = world.comp::<Brain>(id) else { continue };
            let held = world.has::<Sentence>(id) || b.emigrating || b.cuffed_by.is_some();
            set_lod(world, id, if held { Lod::Coarse } else { forced });
        }
    } else {
        assign(world);
    }
}

/// Rank every living adult and hand out the tiers with hysteresis. Jailed
/// and emigrating agents are Coarse without taking a slot.
fn assign(world: &mut World) {
    let view = world.view_rect;
    let centre = match view {
        Some(r) => TilePos { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        None => TilePos { x: (MAP_W / 2) as u8, y: (MAP_H / 2) as u8 },
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
        let priority = i32::from(on_screen(pos.tile)) * 3 + i32::from(brain.pinned || gang) * 2 + i32::from(story);
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
        if let Some(b) = world.comp_mut::<Brain>(id) {
            b.lod = lod;
            b.exec = ExecState::Idle;
            b.current_goal = None;
        }
        world.retier(id);
        snap_to_phase_door(world, id, true);
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
                world.building_of_kind(BuildingKind::Bar)
            } else {
                world.building_of_kind(BuildingKind::Market)
            }
        }
        DayPhase::Evening => home,
    }
    .or_else(|| world.building_of_kind(BuildingKind::Market));
    if let Some(b) = building {
        world.stand_at_door(id, b);
    }
}

/// Which table row the current phase uses.
fn stat_row(world: &World) -> Option<StatRow> {
    let t = world.stat_table.as_ref()?;
    Some(match world.phase() {
        DayPhase::Morning => t.morning.clone(),
        DayPhase::Work => t.work.clone(),
        DayPhase::Evening => t.evening.clone(),
        DayPhase::Night => t.night.clone(),
    })
}

/// Statistical agents whose hourly slot is this tick: `id.index % 60 == tick % 60`.
/// Each agent keeps a fixed minute of the hour, so it runs 24 times a day and
/// the load is spread evenly over the ticks.
pub fn due_this_tick(world: &World) -> Vec<EntityId> {
    let slot = world.tick % TICKS_PER_HOUR;
    world
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .filter(|id| u64::from(id.index) % TICKS_PER_HOUR == slot)
        .filter(|&id| !world.has::<Sentence>(id))
        .collect()
}

/// The hourly stand-in for a Statistical brain: an hour of need decay, then
/// one outcome drawn from the calibrated row with the agent's own stream. It
/// runs every tick for the agents due this tick (`due_this_tick`).
pub fn run_statistical(world: &mut World) {
    let agents = due_this_tick(world);
    if agents.is_empty() {
        return;
    }
    let Some(row) = stat_row(world) else {
        panic!("assets/stat_table.toml is missing: run `cargo run -p citysim-cli -- calibrate`");
    };
    let cfg = world.config.needs.clone();
    let season = world.season().index();
    let season_energy_mult = world.config.economy.energy_decay_mult[season];
    let phase = world.phase();
    for id in agents {
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
        world.recompute_wealth_for(id);
    }
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
        return;
    }
    let Some((market, stock, price)) = world.building_of_kind(BuildingKind::Market).and_then(|m| {
        world.comp::<Building>(m).map(|b| (m, b.stock_food, world.market().map_or(1, |mk| mk.price_food)))
    }) else {
        return;
    };
    if stock == 0 {
        return;
    }
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    if coins >= price {
        // The same purchase a Full agent makes: pay, take, eat.
        let paid = economy::pay_for_food(world, id, 1);
        if economy::take_food(world, id, 1, paid) {
            if let Some(i) = world.comp_mut::<crate::components::Inventory>(id) {
                i.food = i.food.saturating_sub(1);
            }
            if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                crate::needs::eat(n, &cfg);
            }
        }
        return;
    }
    // Theft is a hungry agent's resort, as the planner's cost table makes it
    // for Full agents (steal_starving_bonus): the lawless steal when hungry,
    // not for a discretionary meal.
    let lawless = world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < 0.3);
    let hungry = world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger < 0.4);
    if !lawless || !hungry {
        return;
    }
    if let Some(b) = world.comp_mut::<Building>(market) {
        b.stock_food -= 1;
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        crate::needs::eat(n, &cfg);
    }
    world.stats.current.thefts += 1;
    let name = world.name_of(id);
    world.push_event(EventKind::Theft, &[id], format!("{name} stole food (off screen)"));
    let caught: f64 = world.rng.agent(id).random();
    if caught < world.config.crime.stat_theft_caught_p {
        crate::systems::law::file_report(world, Crime::Theft, id, None);
        // The normal arrest path needs a body on the map.
        set_lod(world, id, Lod::Coarse);
    }
}

fn stat_work(world: &mut World, id: EntityId, phase: DayPhase) {
    let tick = world.tick;
    let Some(job) = world.comp::<Job>(id).cloned() else {
        // The dole, paid directly, once a day in the Work phase.
        if phase == DayPhase::Work && crate::systems::demography::is_adult(world, id) {
            economy::collect_dole(world, id);
        }
        return;
    };
    let key = job.shift_key_at(tick);
    if !job.on_shift(world.tick_of_day()) || !crate::exec::routine::is_workday(key) {
        return;
    }
    if job.role == Role::Farmer {
        if let Some(farm) = job.employer {
            economy::accrue_farm_work(world, id, farm, u64::from(TICKS_PER_HOUR as u32));
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

fn stat_social(world: &mut World, id: EntityId) {
    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
        n.belonging = (n.belonging + 0.1).min(1.0);
    }
    let neighbours: Vec<EntityId> = world.neighbours(id).collect();
    if neighbours.is_empty() {
        // Nobody known yet: meet a Statistical housemate.
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let mates: Vec<EntityId> = world
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .filter(|&o| o != id && world.comp::<Household>(o).and_then(|h| h.home) == home && home.is_some())
            .collect();
        if !mates.is_empty() {
            let k = world.rng.agent(id).random_range(0..mates.len());
            let tick = world.tick;
            let e = world.edge_entry(id, mates[k]);
            e.last_interaction = tick;
        }
        return;
    }
    let k = world.rng.agent(id).random_range(0..neighbours.len());
    crate::systems::social::adjust(world, id, neighbours[k], 0.02, 0.0);
}
