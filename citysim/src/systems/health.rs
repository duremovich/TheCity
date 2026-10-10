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
const ROLL_CANCER: u64 = 0x51c4_0002;
const ROLL_TREAT: u64 = 0x51c4_0003;

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
    let tick = world.tick;
    let (mut dying, mut diagnosed, mut cancer_new) = (Vec::new(), Vec::new(), 0u32);
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
        let age = age_years(world, id);
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
        // A share cleared a day: each diet settles at its steady level.
        let tox =
            h.toxicity + cfg.tox_synth * (f32::from(t.synth) + f32::from(t.alms)) + cfg.tox_dive * f32::from(t.dive)
                - cfg.tox_clear_frac * h.toxicity;
        h.toxicity = tox.clamp(0.0, 1.0);
        // § 3: the cancer's onset and course (adults only, decision 6).
        if cfg.cancer {
            match h.cancer.as_mut() {
                Some(c) => c.stage = (c.stage + cfg.cancer_growth).min(1.0),
                None if h.toxicity > 0.0 => {
                    let p = hazard(&cfg, h.toxicity, age);
                    let roll = crate::systems::econ::hash_unit(seed, ROLL_CANCER, u64::from(id.index), day);
                    if roll < p {
                        h.cancer = Some(crate::components::Cancer { stage: 0.0, since: tick, diagnosed: false });
                        cancer_new += 1;
                    }
                }
                None => {}
            }
            if let Some(c) = h.cancer.as_mut() {
                if c.stage >= 1.0 {
                    dying.push(id);
                } else if !c.diagnosed && c.stage >= cfg.cancer_symptom {
                    c.diagnosed = true;
                    diagnosed.push(id);
                }
            }
        }
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
    for &id in &diagnosed {
        let name = world.name_of(id);
        world.push_event(crate::events::EventKind::Diagnosed, &[id], format!("{name} has a vat-cancer"));
        world.remember(id, crate::components::MemoryKind::Diagnosed, None, 0.9, -0.8, false);
    }
    for id in dying {
        world.kill(id, crate::components::DeathCause::Disease);
    }
    // § 3: a diagnosed patient off screen treats on `p_stat_treat` of days.
    if cfg.cancer {
        stat_treat(world, &cfg, seed, day);
    }
    world.stats.current.health.cancer_new += cancer_new;
    world.stats.current.health.cancer_diagnosed += diagnosed.len() as u32;
    for id in fire {
        if !crate::systems::law::living(world, id) {
            continue;
        }
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

/// § 3: the cancer's daily hazard at `toxicity` and `age` sim years: 0 at
/// zero toxicity, rising with toxicity and (past `cancer_age_from`) with age.
pub fn hazard(cfg: &crate::config::SicknessCfg, toxicity: f32, age: f32) -> f32 {
    (cfg.cancer_hazard * toxicity.max(0.0) * age_factor(cfg, age)).min(1.0)
}

/// § 3: `1 + max(0, age - cancer_age_from) / cancer_age_span`.
pub fn age_factor(cfg: &crate::config::SicknessCfg, age: f32) -> f32 {
    1.0 + (age - cfg.cancer_age_from).max(0.0) / cfg.cancer_age_span.max(1e-6)
}

/// The agent's age in sim years.
pub fn age_years(world: &World, id: EntityId) -> f32 {
    world.comp::<crate::components::Identity>(id).map_or(0.0, |i| i.age_days as f32 / crate::time::DAYS_PER_YEAR as f32)
}

/// `econ::hash_unit` purposes of the history seeding.
const ROLL_SEED_TOX: u64 = 0x51c4_0004;
const ROLL_SEED_CANCER: u64 = 0x51c4_0005;
const ROLL_SEED_STAGE: u64 = 0x51c4_0006;

/// The synth share of the diet an adult likely eats (world creation, an
/// arrival): a Dreg all synth and scraps; housed, by its coins against a
/// fortnight of real meals, the Spire at most `0.2`.
fn likely_synth_share(world: &World, id: EntityId) -> f32 {
    let home = world.comp::<crate::components::Household>(id).and_then(|h| h.home);
    let Some(home) = home else { return 1.0 };
    let tier = world.comp::<crate::components::Building>(home).map_or(1, |b| b.tier);
    let p = crate::systems::life::meal_price(world, id).max(1) as f32;
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins).max(0) as f32;
    let s = 1.0 - 0.9 * (coins / (14.0 * p)).min(1.0);
    match tier {
        0 => s,
        1 => s * 0.7,
        _ => (s * 0.3).min(0.2),
    }
}

/// The steady toxicity of a synth share `s` at one day's food (one meal a
/// day, the sim's measured rate): `tox_synth x s / tox_clear_frac`; a Dreg's
/// scraps add a tenth of a day in bin meals.
fn steady_tox(cfg: &crate::config::SicknessCfg, s: f32, dreg: bool) -> f32 {
    let dives = if dreg { 0.1 } else { 0.0 };
    ((cfg.tox_synth * s + cfg.tox_dive * dives) / cfg.tox_clear_frac.max(1e-6)).min(1.0)
}

/// Dylan, after P5: people start with a history. An adult's toxicity is the
/// steady level of its likely diet, approached with age (the young below
/// it); a Dreg's malnutrition is the synth cap approached the same way; and
/// a cancer is already in progress on the hazard's share (prevalence =
/// the daily hazard x the days an early cancer stays silent, its stage
/// uniform below the symptom). Pure hashes of the seed and the agent:
/// deterministic, the same at either LOD tier.
pub fn seed_agent(world: &mut World, id: EntityId) {
    if !on(world) || !crate::systems::demography::is_adult(world, id) || !world.has::<Needs>(id) {
        return;
    }
    let cfg = world.config.sickness.clone();
    let seed = world.seed();
    let key = u64::from(id.index);
    let dreg = world.comp::<crate::components::Household>(id).is_none_or(|h| h.home.is_none());
    let s = likely_synth_share(world, id);
    let age = age_years(world, id);
    let approach = 1.0 - (-(age - 16.0).max(0.0) / cfg.tox_seed_years.max(1e-6)).exp();
    let u = crate::systems::econ::hash_unit(seed, ROLL_SEED_TOX, key, 0);
    let jitter = 1.0 - cfg.tox_seed_jitter / 2.0 + cfg.tox_seed_jitter * u;
    let tox = (steady_tox(&cfg, s, dreg) * approach * jitter).clamp(0.0, 1.0);
    let mal = if dreg { (cfg.synth_malnut_cap * approach * jitter).min(1.0) } else { 0.0 };
    let mut cancer = None;
    if cfg.cancer {
        let silent_days = cfg.cancer_symptom / cfg.cancer_growth.max(1e-6);
        let prevalence = hazard(&cfg, tox, age) * silent_days;
        if crate::systems::econ::hash_unit(seed, ROLL_SEED_CANCER, key, 0) < prevalence {
            let stage = cfg.cancer_symptom * crate::systems::econ::hash_unit(seed, ROLL_SEED_STAGE, key, 0);
            cancer = Some(crate::components::Cancer { stage, since: world.tick, diagnosed: false });
        }
    }
    let Some(h) = health_mut(world, id) else { return };
    h.toxicity = tox;
    h.malnutrition = h.malnutrition.max(mal);
    if h.cancer.is_none() {
        h.cancer = cancer;
    }
}

/// World creation: every adult's history (`seed_agent`).
pub fn seed_history(world: &mut World) {
    if !on(world) {
        return;
    }
    for id in world.citizens() {
        seed_agent(world, id);
    }
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

// ---------------------------------------------------------------------------
// § 3: cancer and the Clinic
// ---------------------------------------------------------------------------

/// The cancer's treatment price (`[sickness] cancer_treat_price`).
pub fn treat_price(world: &World) -> i64 {
    world.config.sickness.cancer_treat_price
}

/// Who pays for `id`'s treatment and how much each holds, in order
/// (decision 3): the patient's wallet, then the spouse and kin (Spouse,
/// Parent and Family edges, ascending id), then the patient's gang.
pub fn payers(world: &World, id: EntityId) -> Vec<(EntityId, i64)> {
    use crate::components::{RelKind, Wallet};
    let mut out = vec![(id, world.comp::<Wallet>(id).map_or(0, |w| w.coins.max(0)))];
    let mut kin: Vec<EntityId> = world
        .neighbours(id)
        .filter(|&o| {
            crate::systems::law::living(world, o)
                && world
                    .edge(id, o)
                    .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family))
        })
        .collect();
    kin.sort_unstable();
    if let Some(s) = world.spouse_of(id) {
        kin.retain(|&k| k != s);
        kin.insert(0, s);
    }
    for k in kin {
        out.push((k, world.comp::<Wallet>(k).map_or(0, |w| w.coins.max(0))));
    }
    if let Some(g) = world.gang_of(id) {
        out.push((g, world.purse(Some(g)).max(0)));
    }
    out
}

/// The coins the pool can put up for `id`.
pub fn pool(world: &World, id: EntityId) -> i64 {
    payers(world, id).iter().map(|&(_, c)| c).sum()
}

/// A diagnosed cancer the pool can pay for, a Clinic open.
pub fn wants_oncology(world: &World, id: EntityId) -> bool {
    if !on(world) || !world.config.sickness.cancer {
        return false;
    }
    let diagnosed = world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed);
    diagnosed && pool(world, id) >= treat_price(world) && clinic_for(world, id, true).is_some()
}

/// A diagnosed patient whose pool falls short of the treatment: saving for
/// it (`life::broke` reads it, so Earn stays alive: work, beg, scavenge).
pub fn saving_short(world: &World, id: EntityId) -> bool {
    on(world)
        && world.config.sickness.cancer
        && world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed)
        && pool(world, id) < treat_price(world)
}

/// The coins a diagnosed patient whose pool falls short saves toward
/// (Earn's savings line; 0 otherwise).
pub fn treatment_savings(world: &World, id: EntityId) -> i64 {
    if !on(world) || !world.config.sickness.cancer {
        return 0;
    }
    let diagnosed = world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed);
    if diagnosed {
        treat_price(world)
    } else {
        0
    }
}

/// The nearest Clinic (`open`: open now, for a body's walk).
fn clinic_for(world: &World, id: EntityId, open: bool) -> Option<EntityId> {
    let tile = world.comp::<crate::components::Position>(id)?.tile;
    crate::systems::assets::nearest_seller(world, crate::components::BuildingKind::Clinic, tile, open)
}

/// The Treat goal's scripted plan for a cancer: `[GoTo(Clinic) c, Oncology c]`.
pub fn oncology_plan(world: &World, id: EntityId) -> Option<crate::goap::Plan> {
    if !wants_oncology(world, id) {
        return None;
    }
    let c = clinic_for(world, id, true)?;
    let step = |action, target| crate::components::ActionInstance { action, target, tile: None };
    let steps = vec![
        step(crate::goap::ActionKind::GoTo(crate::goap::LocationKey::Clinic), Some(c)),
        step(crate::goap::ActionKind::Oncology, Some(c)),
    ];
    Some(crate::goap::Plan {
        goal: crate::components::GoalKind::Treat,
        target: Some(c),
        steps,
        started_tick: world.tick,
    })
}

/// `Oncology`'s start check: inside the Clinic, still wanting it.
pub fn can_start_oncology(world: &World, id: EntityId, target: Option<EntityId>) -> bool {
    let here = world.comp::<crate::components::Position>(id).and_then(|p| p.building);
    target.is_some() && here == target && !target.is_some_and(|c| world.is_closed(c)) && wants_oncology(world, id)
}

/// The treatment at `clinic` (both tiers): the price from the pool in order
/// (`Flow::Treatment` to the Clinic's owner); below `cure_stage` a cure
/// (toxicity x `treat_tox_mult`), else remission (`stage - remission`).
pub fn oncology(world: &mut World, id: EntityId, clinic: EntityId) -> bool {
    use crate::systems::ownership::{self, Flow};
    let price = treat_price(world);
    let diagnosed = world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed);
    if !on(world) || !diagnosed || pool(world, id) < price {
        return false;
    }
    let to = world.owner_of(clinic);
    let mut due = price;
    for (payer, holds) in payers(world, id) {
        if due <= 0 {
            break;
        }
        let take = holds.min(due);
        if take > 0 {
            let paid = ownership::pay(world, Some(payer), to, take, Flow::Treatment);
            ownership::credit(world, clinic, paid);
            due -= paid;
            if payer != id {
                world.remember(payer, crate::components::MemoryKind::Paid, Some(id), 0.4, -0.1, false);
            }
        }
    }
    let cfg = world.config.sickness.clone();
    let (who, at) = (world.name_of(id), world.name_of(clinic));
    let Some(h) = world.comp_mut::<Health>(id) else { return false };
    let Some(c) = h.cancer.as_mut() else { return false };
    let cured = c.stage < cfg.cure_stage;
    if cured {
        h.cancer = None;
        h.toxicity *= cfg.treat_tox_mult;
    } else {
        c.stage = (c.stage - cfg.remission).max(0.0);
    }
    world.stats.current.treatments += 1;
    if cured {
        world.stats.current.health.cancer_cured += 1;
        world.push_event(
            crate::events::EventKind::Cured,
            &[id, clinic],
            format!("{who} was cured of a vat-cancer at {at}"),
        );
    } else {
        world.push_event(
            crate::events::EventKind::Treated,
            &[id, clinic],
            format!("{who} bought a remission of a vat-cancer at {at}"),
        );
    }
    true
}

/// Off screen: a diagnosed Statistical patient whose pool pays treats on
/// `p_stat_treat` of days at the nearest open Clinic (the `p_stat_detox`
/// pattern: too rare for `calibrate` to learn).
fn stat_treat(world: &mut World, cfg: &crate::config::SicknessCfg, seed: u64, day: u64) {
    let due: Vec<EntityId> = world
        .tier(crate::components::Lod::Statistical)
        .iter()
        .copied()
        .filter(|&id| world.comp::<Health>(id).and_then(|h| h.cancer).is_some_and(|c| c.diagnosed))
        .collect();
    for id in due {
        if pool(world, id) < treat_price(world) {
            continue;
        }
        let roll = crate::systems::econ::hash_unit(seed, ROLL_TREAT, u64::from(id.index), day);
        if roll >= cfg.p_stat_treat {
            continue;
        }
        if let Some(c) = clinic_for(world, id, false) {
            oncology(world, id, c);
        }
    }
}
