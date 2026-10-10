//! Hunger as sickness § 2-3 (docs/HUNGER_SICKNESS.md): the malnutrition
//! and toxicity meters, the day's meal tally, the debuffs, the sick days.
//!
//! Both LOD tiers keep the tally the same way (every eat site calls [`ate`];
//! the hungry hours are marked hourly on bodies in `needs::run` and on
//! Statistical agents in `lod::run_statistical`), and [`daily`] runs at
//! midnight over every citizen whatever its LOD: the meters, the sick days
//! and every debuff are identical across tiers by construction. Every roll
//! is a pure hash (`econ::hash_unit`), never a stream draw.

use crate::components::{Child, Health, Job, MealKind, Needs, Sentence};
use crate::entity::EntityId;
use crate::world::World;

/// `econ::hash_unit` purposes.
const ROLL_SICK: u64 = 0x51c4_0001;

/// The meters and debuffs run (`[sickness] enabled` and `diet`).
pub fn on(world: &World) -> bool {
    crate::systems::diet::on(world)
}

/// The agent's malnutrition (0 without a `Health`).
pub fn malnutrition(world: &World, id: EntityId) -> f32 {
    world.comp::<Health>(id).map_or(0.0, |h| h.malnutrition)
}

/// The agent's `Health`, made on first touch (only with the diet on).
fn health_mut(world: &mut World, id: EntityId) -> Option<&mut Health> {
    if !world.has::<Health>(id) {
        if !world.has::<Needs>(id) && !world.has::<Child>(id) {
            return None;
        }
        world.insert(id, Health::default());
    }
    world.comp_mut::<Health>(id)
}

/// One meal of `kind` eaten by `id` (both tiers; a no-op with the diet off).
pub fn ate(world: &mut World, id: EntityId, kind: MealKind) {
    if !on(world) {
        return;
    }
    let Some(h) = health_mut(world, id) else { return };
    let t = &mut h.today;
    let slot = match kind {
        MealKind::Real => &mut t.real,
        MealKind::Synth => &mut t.synth,
        MealKind::Dive => &mut t.dive,
        MealKind::Alms => &mut t.alms,
    };
    *slot = slot.saturating_add(1);
    let row = &mut world.stats.current.health;
    match kind {
        MealKind::Real => row.meals_real += 1,
        MealKind::Synth => row.meals_synth += 1,
        MealKind::Dive => row.meals_dive += 1,
        MealKind::Alms => row.meals_alms += 1,
    }
}

/// The kind of the unit eaten from a store holding `food` units of which
/// `synth` are synth: real first.
pub fn kind_of_unit(food: u32, synth: u32) -> MealKind {
    if food > synth.min(food) {
        MealKind::Real
    } else {
        MealKind::Synth
    }
}

/// One unit eaten out of an inventory: `food -= 1`, the synth subset
/// clamped (real first); returns the meal's kind.
pub fn eat_inventory_unit(inv: &mut crate::components::Inventory) -> MealKind {
    let synth = inv.synth_held();
    let kind = kind_of_unit(inv.food, synth);
    inv.food = inv.food.saturating_sub(1);
    inv.synth = synth.min(inv.food);
    kind
}

/// One unit eaten out of a pantry (real first); returns the meal's kind.
pub fn eat_pantry_unit(b: &mut crate::components::Building) -> MealKind {
    let synth = b.synth_food.min(b.stock_food);
    let kind = kind_of_unit(b.stock_food, synth);
    b.stock_food = b.stock_food.saturating_sub(1);
    b.synth_food = synth.min(b.stock_food);
    kind
}

/// The hourly mark: an hour below `mal_hungry_edge` is a hungry hour.
pub fn hourly_mark(world: &mut World, id: EntityId) {
    if !on(world) {
        return;
    }
    let edge = world.config.sickness.mal_hungry_edge;
    if world.comp::<Needs>(id).is_some_and(|n| n.hunger < edge) {
        if let Some(h) = health_mut(world, id) {
            h.today.hungry_hours = h.today.hungry_hours.saturating_add(1);
        }
    }
}

/// A child's day (`demography::children`): fed from the pantry (synth or
/// real) or unfed. Children carry malnutrition only (decision 6).
pub fn child_day(world: &mut World, child: EntityId, fed: bool, synth: bool) {
    if !on(world) {
        return;
    }
    let cfg = world.config.sickness.clone();
    let Some(h) = health_mut(world, child) else { return };
    let d = if !fed {
        cfg.mal_hungry
    } else if synth {
        cfg.mal_poor_diet.min((cfg.synth_malnut_cap - h.malnutrition).max(0.0))
    } else {
        -cfg.mal_real
    };
    h.malnutrition = (h.malnutrition + d).clamp(0.0, 1.0);
}

/// Midnight (`economy::run`, after spoilage): every adult citizen's meters
/// from yesterday's tally, the tally reset, today's sick roll and the
/// sick-day dismissal (decision 4); the day's columns.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.sickness.clone();
    let day = world.day();
    let seed = world.seed();
    let mut fire = Vec::new();
    let (mut malnourished, mut toxic, mut sick, mut n) = (0u32, 0u32, 0u32, 0u32);
    let (mut sum_m, mut sum_t) = (0.0f32, 0.0f32);
    for id in world.citizens() {
        if world.has::<Child>(id) || !world.has::<Needs>(id) {
            if let Some(h) = world.comp::<Health>(id) {
                n += 1;
                sum_m += h.malnutrition;
                malnourished += u32::from(h.malnutrition >= cfg.mal_onset);
            }
            continue;
        }
        let employed = world.comp::<Job>(id).is_some() && !world.has::<Sentence>(id);
        let Some(h) = health_mut(world, id) else { continue };
        let t = std::mem::take(&mut h.today);
        let synthy = f32::from(t.synth) + f32::from(t.alms);
        let meals = synthy + f32::from(t.dive) + f32::from(t.real);
        // A synth diet levels off at `synth_malnut_cap` (coordinator call on
        // P2); hunger and bin scraps push past it.
        let synth_rise = cfg.mal_poor_diet * synthy / meals.max(1.0);
        let synth_rise = synth_rise.min((cfg.synth_malnut_cap - h.malnutrition).max(0.0));
        let m = h.malnutrition
            + cfg.mal_hungry * f32::from(t.hungry_hours.min(24)) / 24.0
            + synth_rise
            + cfg.mal_poor_diet * f32::from(t.dive) / meals.max(1.0)
            - cfg.mal_real * f32::from(t.real.min(2));
        h.malnutrition = m.clamp(0.0, 1.0);
        let tox =
            h.toxicity + cfg.tox_synth * (f32::from(t.synth) + f32::from(t.alms)) + cfg.tox_dive * f32::from(t.dive)
                - cfg.tox_clear;
        h.toxicity = tox.clamp(0.0, 1.0);
        // Today's sick roll (a pure hash: no stream moves).
        let m_on = onset(h.malnutrition, cfg.mal_onset);
        let c = h.cancer.map_or(0.0, |c| c.stage);
        let p = (cfg.sick_mal * m_on + c).clamp(0.0, 1.0);
        let roll = crate::systems::econ::hash_unit(seed, ROLL_SICK, u64::from(id.index), day);
        h.sick_today = employed && p > 0.0 && roll < p;
        h.sick_days = (h.sick_days << 1) & 0x3fff;
        if h.sick_today {
            h.sick_days |= 1;
        }
        let sick_n = h.sick_days.count_ones();
        n += 1;
        sum_m += h.malnutrition;
        sum_t += h.toxicity;
        malnourished += u32::from(h.malnutrition >= cfg.mal_onset);
        toxic += u32::from(h.toxicity >= cfg.tox_onset);
        if h.sick_today {
            sick += 1;
        }
        if employed && h.sick_today && sick_n >= cfg.sick_fire_days {
            h.sick_days = 0;
            fire.push(id);
        }
    }
    for id in fire {
        let building = world.comp::<Job>(id).and_then(|j| j.employer);
        let name = world.name_of(id);
        crate::systems::economy::dismiss(world, id, building, format!("{name} was let go: too sick to work"));
    }
    let row = &mut world.stats.current.health;
    row.malnourished = malnourished;
    row.toxic = toxic;
    row.sick_days = sick;
    row.mean_malnutrition = if n > 0 { sum_m / n as f32 } else { 0.0 };
    row.mean_toxicity = if n > 0 { sum_t / n as f32 } else { 0.0 };
}

/// `max(0, x − onset) / (1 − onset)`.
fn onset(x: f32, onset: f32) -> f32 {
    ((x - onset).max(0.0) / (1.0 - onset).max(1e-6)).min(1.0)
}

/// Is `id` off sick today (no shift, no wage)?
pub fn sick(world: &World, id: EntityId) -> bool {
    on(world) && world.comp::<Health>(id).is_some_and(|h| h.sick_today)
}

/// The debuffs of § 2 (neutral with the diet off or no `Health`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// Energy decay x this.
    pub energy_mult: f32,
    /// Work output x this.
    pub work_mult: f32,
    /// Added to mood's target.
    pub mood_bias: f32,
    /// Off the fight's skill term.
    pub frail_fight: f32,
    /// The old-age hazard x this.
    pub old_mult: f32,
    /// Conception possible, and its chance x this.
    pub fertile: bool,
    pub conceive_mult: f32,
}

impl Effects {
    pub const NONE: Effects = Effects {
        energy_mult: 1.0,
        work_mult: 1.0,
        mood_bias: 0.0,
        frail_fight: 0.0,
        old_mult: 1.0,
        fertile: true,
        conceive_mult: 1.0,
    };
}

pub fn effects(world: &World, id: EntityId) -> Effects {
    if !on(world) {
        return Effects::NONE;
    }
    let Some(h) = world.comp::<Health>(id) else { return Effects::NONE };
    let cfg = &world.config.sickness;
    let (m, t) = (h.malnutrition, h.toxicity);
    let mp = onset(m, cfg.mal_onset);
    let tp = onset(t, cfg.tox_onset);
    let c = h.cancer.map_or(0.0, |c| c.stage);
    Effects {
        energy_mult: 1.0 + cfg.mal_energy * mp + cfg.tox_energy * tp + cfg.cancer_energy * c,
        work_mult: (1.0 - cfg.mal_work * mp - cfg.cancer_work * c).max(0.3),
        mood_bias: -(cfg.mal_mood * m + cfg.tox_mood * t + cfg.cancer_mood * c),
        frail_fight: cfg.frail_fight * (mp + c),
        old_mult: 1.0 + cfg.frail_old * (m + t),
        fertile: m < cfg.fertile_mal_max && h.cancer.is_none(),
        conceive_mult: (1.0 - t).max(0.0),
    }
}

/// `DecayCtx.energy_mult` composed with the body's energy debuff: `prev`
/// unchanged with the diet off or a neutral body.
pub fn energy_mult(world: &World, id: EntityId, prev: Option<f32>) -> Option<f32> {
    let e = effects(world, id).energy_mult;
    if e == 1.0 {
        prev
    } else {
        Some(prev.unwrap_or(1.0) * e)
    }
}

/// Work output x this (1 with the diet off).
pub fn work_mult(world: &World, id: EntityId) -> f32 {
    effects(world, id).work_mult
}
