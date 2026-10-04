//! Level of detail. Every 60 ticks the highest-priority agents become Full,
//! the next band Coarse and the rest Statistical; a calibrated hourly table
//! (`assets/stat_table.toml`, written by `citysim-cli calibrate`) stands in
//! for the brain of a Statistical agent.

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
            | ActionKind::BuryCorpse
    )
}

pub fn run(world: &mut World) {
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    if let Some(forced) = world.config.lod.force {
        for id in world.citizens() {
            if world.has::<Brain>(id) {
                set_lod(world, id, forced);
            }
        }
    } else {
        assign(world);
    }
    run_statistical(world);
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
    for id in world.citizens() {
        let (Some(pos), Some(brain)) = (world.comp::<Position>(id), world.comp::<Brain>(id)) else { continue };
        if world.has::<Sentence>(id) || brain.emigrating {
            set_lod(world, id, Lod::Coarse);
            continue;
        }
        let story = brain.current_step().is_some_and(|s| story_relevant(s.action));
        let priority = i32::from(on_screen(pos.tile)) * 3 + i32::from(brain.pinned) * 2 + i32::from(story);
        ranked.push((-priority, pos.tile.manhattan(centre), id.index, id));
    }
    ranked.sort_unstable();
    let ids: Vec<EntityId> = ranked.iter().map(|&(_, _, _, id)| id).collect();
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
    // tier, and the lowest-ranked newcomer above the line is held back.
    for (upper, boundary) in [(Lod::Full, max_full), (Lod::Coarse, max_full + max_coarse)] {
        let lower = if upper == Lod::Full { Lod::Coarse } else { Lod::Statistical };
        let band_end = (boundary + HYSTERESIS_BAND).min(ids.len());
        for rank in boundary..band_end {
            if current[rank] != upper || tier[rank] == upper {
                continue;
            }
            // The lowest-ranked newcomer (currently in a lower tier) above the line.
            let newcomer = (0..boundary).rev().find(|&r| tier[r] == upper && current[r] != upper);
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
    let Some(door) = building.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) else { return };
    world.remove_from_building(id);
    let tick = world.tick;
    if let Some(p) = world.comp_mut::<Position>(id) {
        p.tile = door;
        p.building = None;
        p.entered = tick;
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

/// The hourly stand-in for a Statistical brain: an hour of need decay, then
/// one outcome drawn from the calibrated row with the agent's own stream.
pub fn run_statistical(world: &mut World) {
    let agents: Vec<EntityId> = world
        .citizens()
        .into_iter()
        .filter(|&id| world.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical))
        .filter(|&id| !world.has::<Sentence>(id))
        .collect();
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
        // 2b. Work is not a gamble: an employed agent on shift works the hour
        // (a Full farmer works every hour of the shift), and the jobless draw
        // the dole once a day. The table's p_work mass then stands for the
        // idle hours of the employed.
        stat_work(world, id, phase);
        // 3. One outcome. A Full agent eats when hungry, not by lottery, so a
        // hungry Statistical agent eats if it can and the draw covers the
        // discretionary meals of the fed.
        let hungry = world.comp::<crate::components::Needs>(id).is_some_and(|n| n.hunger < 0.4);
        let u: f32 = world.rng.agent(id).random();
        let outcome = if hungry || u < row.p_eat {
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
    }
    world.recompute_wealth();
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
        if let Some(w) = world.comp_mut::<crate::components::Wallet>(id) {
            w.coins -= price;
        }
        if let Some(t) = world.treasury_mut() {
            t.coins += price;
        }
        if let Some(b) = world.comp_mut::<Building>(market) {
            b.stock_food -= 1;
        }
        if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
            crate::needs::eat(n, &cfg);
        }
        return;
    }
    let lawless = world.comp::<Personality>(id).is_some_and(|p| p.lawfulness < 0.3);
    if !lawless {
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
    if !job.on_shift(world.tick_of_day()) {
        return;
    }
    if job.role == Role::Farmer {
        if let Some(farm) = job.employer {
            economy::accrue_farm_work(world, id, farm, u64::from(TICKS_PER_HOUR as u32));
        }
    }
    let key = job.shift_key_at(tick);
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
            .citizens()
            .into_iter()
            .filter(|&o| o != id && world.comp::<Household>(o).and_then(|h| h.home) == home && home.is_some())
            .filter(|&o| world.comp::<Brain>(o).is_some_and(|b| b.lod == Lod::Statistical))
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
