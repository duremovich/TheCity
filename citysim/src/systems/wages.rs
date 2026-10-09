//! The Real economy phase 2 (docs/ECONOMY_V2.md §§ 1, 5; plan E13-E20):
//! wages from revenue. Every corp's payroll tracks a labour share of its
//! 7-day revenue through `Corp.wage_rev` (E17), a corp under its target
//! posts a vacancy a building a day and one over it for a week lays off
//! its newest hire (E18), a producer pays inputs to the World per unit
//! made and its buildings a daily power charge (E13), and a non-city
//! owner pays the Treasury a property rate instead of M11's upkeep (E13).
//! Everything here is a ledger abstraction: coins moved between integer
//! purses by `ownership::{charge, cross_out}`, counters on `Corp`, and
//! vacancies on `world.vacancies`. No path runs without [`on`]; nothing
//! here runs per agent per tick (two midnight passes from `living::run`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::components::{Building, BuildingKind, Corp, Job, Niche, Role};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::outside::{ExportGood, WORLD_ACCOUNT};
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// Days of `Corp.{rev, pay}` (E16).
pub const WINDOW_DAYS: usize = 7;
/// E18: days under `hire_below × P*` before a corp posts.
const HIRE_DAYS: u8 = 3;
/// E18: days over `fire_above × P*` before a corp lays off.
const FIRE_DAYS: u8 = 7;

/// Plan E1: `[living] enabled && [economy2] enabled && [economy2] wages`.
pub fn on(world: &World) -> bool {
    crate::systems::econ::wages_on(world)
}

// ---------------------------------------------------------------------------
// Inputs and the property rate (E13)
// ---------------------------------------------------------------------------

/// E13: `amount` coins of inputs from `from` to the World (`Flow::Inputs`,
/// `cross_out`: a corp or the city pays in full, an agent or gang what its
/// purse holds). Without a World account (the market off) the coins go to
/// the Treasury as M13 D8's imports did (a city payer: a ledger line
/// only), so nothing vanishes. Returns the coins moved.
pub fn inputs(world: &mut World, from: Option<EntityId>, amount: i64) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let full = matches!(ownership::owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    if world.outside.faction(WORLD_ACCOUNT).is_some() {
        ownership::cross_out(world, from, WORLD_ACCOUNT, amount, Flow::Inputs, full)
    } else if from.is_none() {
        ownership::ledger_only(world, Flow::Inputs, amount);
        amount
    } else {
        ownership::charge(world, from, None, amount, Flow::Inputs)
    }
}

/// E13: the inputs of `units` produced at `b` at `per_unit` coins each:
/// the fraction carried in `Building.input_accum`, whole coins crossed out
/// by the building's owner. A no-op with wages off.
pub fn produce_inputs(world: &mut World, b: EntityId, units: u32, per_unit: f32) {
    if !on(world) || units == 0 || per_unit <= 0.0 {
        return;
    }
    let owner = world.owner_of(b);
    let coins = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.input_accum += units as f32 * per_unit;
        let whole = bd.input_accum.floor();
        bd.input_accum -= whole;
        whole as i64
    };
    inputs(world, owner, coins);
}

/// E13: the midnight pass that replaces `ownership::upkeep` with wages on:
/// per standing building, a non-city owner pays the Treasury the property
/// rate (`Flow::Property`; a newly incorporated corp's grace holds), and
/// any owner, the city included, pays the kind's daily power to the World
/// (`Flow::Inputs`). `[corps] upkeep` and the band's `upkeep_mult` are
/// not read.
pub fn upkeep(world: &mut World) {
    let rate = world.config.treasury.property_rate.clone();
    let power = world.config.economy2.power.clone();
    let now = world.tick;
    for b in world.with::<Building>() {
        let Some((kind, tier, owner)) = world
            .comp::<Building>(b)
            .filter(|bd| !bd.demolished && !bd.derelict)
            .map(|bd| (bd.kind, bd.tier, bd.owner))
        else {
            continue;
        };
        let grace =
            owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.upkeep_grace_until).is_some_and(|t| now < t);
        let property = rate.for_building(kind, tier);
        if owner.is_some() && property > 0 && !grace {
            ownership::charge(world, owner, None, property, Flow::Property);
        }
        let p = power.for_kind(kind);
        if p > 0 && !grace {
            inputs(world, owner, p);
        }
    }
}

// ---------------------------------------------------------------------------
// The wage rule (E17)
// ---------------------------------------------------------------------------

fn mean(ring: &VecDeque<i64>) -> f32 {
    if ring.is_empty() {
        0.0
    } else {
        ring.iter().sum::<i64>() as f32 / ring.len() as f32
    }
}

/// E17: the labour share of a corp's revenue: the building-weighted mean
/// of `[economy2] labour_share` over its niches (deviation: the spec's
/// "primary niche" is ambiguous for Vatra and Militech); the plain mean
/// over its niches when it holds no niche building.
pub fn labour_share(world: &World, corp: EntityId) -> f32 {
    let cfg = &world.config.economy2.labour_share;
    let Some(c) = world.comp::<Corp>(corp) else { return 0.0 };
    if c.niches.is_empty() {
        return 0.0;
    }
    let (mut num, mut den) = (0.0f32, 0usize);
    for &n in &c.niches {
        let k = crate::systems::corp_brain::niche_buildings(world, corp, n).len();
        num += cfg.of(n) * k as f32;
        den += k;
    }
    if den == 0 {
        return c.niches.iter().map(|&n| cfg.of(n)).sum::<f32>() / c.niches.len() as f32;
    }
    num / den as f32
}

/// The 7-day means `(R, P)` once both windows are full; `None` before.
pub fn windows(world: &World, corp: EntityId) -> Option<(f32, f32)> {
    let c = world.comp::<Corp>(corp)?;
    (c.rev.len() >= WINDOW_DAYS && c.pay.len() >= WINDOW_DAYS).then(|| (mean(&c.rev), mean(&c.pay)))
}

/// The payroll target `P* = labour_share × R` (`None` before the windows fill).
pub fn target(world: &World, corp: EntityId) -> Option<f32> {
    windows(world, corp).map(|(r, _)| labour_share(world, corp) * r)
}

/// E17: `EconState.vacancy_since` against the open vacancies: a `(building,
/// role)` first seen standing is stamped now, one no longer open is dropped.
fn track_vacancies(world: &mut World) {
    let now = world.tick;
    let mut open: BTreeSet<(EntityId, Role)> = BTreeSet::new();
    for (&b, roles) in &world.vacancies {
        for &r in roles {
            open.insert((b, r));
        }
    }
    world.econ.vacancy_since.retain(|k, _| open.contains(k));
    for k in open {
        world.econ.vacancy_since.entry(k).or_insert(now);
    }
}

/// E17: has a vacancy at one of the corp's buildings stood `shortage_days`?
fn shortage(world: &World, corp: EntityId, days: u64) -> bool {
    let Some(c) = world.comp::<Corp>(corp) else { return false };
    let now = world.tick;
    // `since + days ≤ now`, not `since ≤ now − days`: in the first days the
    // subtraction saturates to 0 and every day-0 vacancy read as a shortage.
    world
        .econ
        .vacancy_since
        .iter()
        .any(|(&(b, _), &since)| since + days * TICKS_PER_DAY <= now && c.buildings.binary_search(&b).is_ok())
}

/// Round to two decimals (0.01 steps stay exact in the CSV and saves).
fn tidy(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}

/// E17: the midnight pass (`living::run`, after `jobs::top_up`): per corp
/// with seven days in its windows and an employee, `wage_rev` steps up when
/// `P < 0.9 P*` or a vacancy stood unfilled `shortage_days`, down when
/// `P > 1.1 P*`, clamped `[wage_floor_mult, wage_cap_mult]`; `WageMoved`
/// when it crosses a 0.1 step. Before the windows fill it holds 1.0.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    track_vacancies(world);
    let cfg = world.config.economy2.clone();
    for corp in world.corps() {
        let Some((r, p)) = windows(world, corp) else { continue };
        if ownership::employees_of(world, corp).is_empty() {
            continue;
        }
        let target = labour_share(world, corp) * r;
        let short = shortage(world, corp, cfg.shortage_days);
        let Some(c) = world.comp::<Corp>(corp) else { continue };
        let old = c.wage_rev;
        let mut new = old;
        if p < 0.9 * target || short {
            new += cfg.wage_step;
        } else if p > 1.1 * target {
            new -= cfg.wage_step;
        }
        new = tidy(new.clamp(cfg.wage_floor_mult, cfg.wage_cap_mult));
        if new == old {
            continue;
        }
        let name = c.name.clone();
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.wage_rev = new;
        }
        let step = |x: f32| (x * 10.0 + 1e-4).floor() as i64;
        if step(new) != step(old) {
            let why = if short { "a vacancy stood unfilled" } else { "payroll against revenue" };
            world.push_event(
                EventKind::WageMoved,
                &[corp],
                format!("{name} pays wages x{new:.2} (was x{old:.2}; payroll {p:.0} a day against a target of {target:.0}: {why})"),
            );
        }
    }
}

/// E20, E45: the payroll-weighted mean `wage_rev` over the corps (each
/// weighted by its 7-day mean payroll; the plain mean when no corp has
/// paid; 1.0 with no corp).
pub fn mean_wage_rev(world: &World) -> f32 {
    let (mut num, mut den, mut plain, mut n) = (0.0f32, 0.0f32, 0.0f32, 0usize);
    for corp in world.corps() {
        let Some(c) = world.comp::<Corp>(corp) else { continue };
        let w = mean(&c.pay).max(0.0);
        num += w * c.wage_rev;
        den += w;
        plain += c.wage_rev;
        n += 1;
    }
    if den > 0.0 {
        num / den
    } else if n > 0 {
        plain / n as f32
    } else {
        1.0
    }
}

/// The gross daily wage a Job pays now (`economy::collect_wage_scaled`'s
/// `per_day` at scale 1): the config wage × the corp's Squeeze `wage_mult`
/// × `wage_rev` × the poaching premium; a city, agent or gang employer's
/// is the config wage × the premium.
pub fn gross_wage(world: &World, job: &Job) -> i64 {
    let payer = job.employer.and_then(|e| world.owner_of(e));
    let corp = payer.and_then(|p| world.comp::<Corp>(p));
    let mult = corp.map_or(1.0, |c| c.wage_mult * c.wage_rev) * job.premium;
    if mult == 1.0 {
        job.wage_per_day
    } else {
        (job.wage_per_day as f32 * mult).round() as i64
    }
}

/// E45: the mean gross daily wage over every Job holder (0 with none).
pub fn gross_mean(world: &World) -> f32 {
    let (mut sum, mut n) = (0i64, 0usize);
    for role in Role::ALL {
        for &a in world.workers(role) {
            if let Some(j) = world.comp::<Job>(a) {
                sum += gross_wage(world, j);
                n += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        sum as f32 / n as f32
    }
}

/// E45: a wallet's inflows other than wages (stipends, a house's pay-out,
/// the scavenge coin, a contract's pay-out, street dice; the robbery take
/// and a repaid loan are counted at their sites), `pop_inflow_other`.
pub fn note_inflow(world: &mut World, to: Option<EntityId>, flow: Flow, net: i64) {
    if net <= 0
        || !matches!(flow, Flow::Tribute | Flow::GambleWin | Flow::Scavenge | Flow::Payout | Flow::StreetDice)
        || !matches!(ownership::owner_kind(world, to), OwnerKind::Agent(_))
    {
        return;
    }
    world.stats.current.econ.pop_inflow_other += net;
}

/// E45: an inflow into a wallet at a site that moves no `Flow` (the
/// robbery take at bind, a repaid loan): `pop_inflow_other` with wages on.
pub fn note_inflow_raw(world: &mut World, coins: i64) {
    if coins > 0 && on(world) {
        world.stats.current.econ.pop_inflow_other += coins;
    }
}

// ---------------------------------------------------------------------------
// Hiring and layoffs (E18)
// ---------------------------------------------------------------------------

/// E18: the unfilled World Food order, the 7-day mean of `cap_today −
/// bought` over the book's rings (0 without a book).
pub fn food_order(world: &World) -> f32 {
    let Some(b) = crate::systems::world_market::book(world, ExportGood::Food) else { return 0.0 };
    let days = crate::systems::world_market::FILL_DAYS;
    let pairs: Vec<(u32, u32)> =
        b.caps.iter().rev().zip(b.bought.iter().rev()).take(days).map(|(&c, &q)| (c, q)).collect();
    if pairs.is_empty() {
        return 0.0;
    }
    pairs.iter().map(|&(c, q)| c.saturating_sub(q) as f32).sum::<f32>() / pairs.len() as f32
}

/// E18: a corp's Farm overtime: `export_staff × unfilled order ÷ 100`
/// places, split across its standing Farms (each gets the rounded share).
fn farm_overtime(world: &World, corp: EntityId, order: f32) -> usize {
    let farms = crate::systems::ownership::owned_of_kind(world, Some(corp), BuildingKind::Farm).len();
    if farms == 0 || order <= 0.0 {
        return 0;
    }
    let places = world.config.economy2.export_staff * order / 100.0;
    (places / farms as f32).round().max(0.0) as usize
}

/// A standing building (not demolished, not derelict).
fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// E18: one vacancy per owned standing building whose niche demand holds
/// `hire_demand`, up to `full_staff` (+ Farm overtime). Returns the vacancies posted.
fn post(world: &mut World, corp: EntityId, order: f32) -> u32 {
    let cfg = world.config.economy2.clone();
    let Some(c) = world.comp::<Corp>(corp) else { return 0 };
    let buildings = c.buildings.clone();
    let niches: Vec<Niche> = c.niches.iter().copied().collect();
    let demand: BTreeMap<Niche, f32> =
        niches.iter().map(|&n| (n, crate::systems::corp_brain::demand_of(world, corp, n).0)).collect();
    let any = demand.values().copied().fold(0.0f32, f32::max);
    let overtime = farm_overtime(world, corp, order);
    let now = world.tick;
    let mut posted = 0;
    for b in buildings {
        if !standing(world, b) {
            continue;
        }
        let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { continue };
        let Some(role) = ownership::role_for(kind) else { continue };
        let d = crate::systems::corp_brain::niche_of_kind(kind).and_then(|n| demand.get(&n).copied()).unwrap_or(any);
        if d < cfg.hire_demand {
            continue;
        }
        let cap =
            crate::systems::corp_brain::full_staff(world, kind) + if kind == BuildingKind::Farm { overtime } else { 0 };
        let working = ownership::staff_at(world, b)
            .into_iter()
            .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == role))
            .count();
        let open = world.vacancies.get(&b).map_or(0, |v| v.iter().filter(|&&r| r == role).count());
        if working + open < cap {
            world.vacancies.entry(b).or_default().push(role);
            world.econ.vacancy_since.entry((b, role)).or_insert(now);
            posted += 1;
        }
    }
    posted
}

/// E18: the corp's newest hire (ties the higher id), its exec excepted and,
/// under `[corps] hunker_spares_farms`, its Farm staff (M11's rule for
/// Hunker: Vat Techs are the city's food; a deviation recorded).
fn newest_hire(world: &World, corp: EntityId) -> Option<(EntityId, EntityId)> {
    let c = world.comp::<Corp>(corp)?;
    let spare_farms = world.config.corps.hunker_spares_farms;
    ownership::employees_of(world, corp)
        .into_iter()
        .filter(|&a| Some(a) != c.exec)
        .filter_map(|a| {
            let j = world.comp::<Job>(a)?;
            let b = j.employer?;
            let kind = world.comp::<Building>(b)?.kind;
            (!(spare_farms && kind == BuildingKind::Farm)).then_some((j.hired_tick, a, b))
        })
        .max()
        .map(|(_, a, b)| (a, b))
}

/// E18: lay the newest hire off (`LaidOff`, "laid off: revenue"; no vacancy).
fn lay_off(world: &mut World, corp: EntityId) -> bool {
    let Some((who, b)) = newest_hire(world, corp) else { return false };
    let what = world.comp::<Job>(who).map_or("worker", |j| j.role.label());
    let (name, cname) = (world.name_of(who), world.owner_label(Some(corp)));
    let text = format!("{name} laid off as {what} by {cname} (laid off: revenue)");
    crate::systems::economy::dismiss_as(world, who, Some(b), text, EventKind::LaidOff);
    world.stats.current.econ.laid_off += 1;
    true
}

/// E18: is the corp over `fire_above × P*` now (it replaces no quitter)?
pub fn over_margin(world: &World, corp: EntityId) -> bool {
    world.comp::<Corp>(corp).is_some_and(|c| c.fire_days >= 1)
}

/// E18: the midnight pass after [`daily`]: per corp with full windows, the
/// day counts against `hire_below` and `fire_above`; at `HIRE_DAYS` under
/// it posts, at `FIRE_DAYS` over it with `wage_rev` at the floor it lays
/// off one (wages fall before jobs go).
pub fn staff(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.economy2.clone();
    let order = food_order(world);
    for corp in world.corps() {
        let Some((r, p)) = windows(world, corp) else { continue };
        let target = labour_share(world, corp) * r;
        let below = p < cfg.hire_below * target;
        let above = p > cfg.fire_above * target;
        let Some(c) = world.comp_mut::<Corp>(corp) else { continue };
        c.hire_days = if below { c.hire_days.saturating_add(1) } else { 0 };
        c.fire_days = if above { c.fire_days.saturating_add(1) } else { 0 };
        let (hire_days, fire_days, wage_rev) = (c.hire_days, c.fire_days, c.wage_rev);
        if hire_days >= HIRE_DAYS {
            post(world, corp, order);
        }
        if fire_days >= FIRE_DAYS && wage_rev <= cfg.wage_floor_mult + 1e-6 {
            lay_off(world, corp);
        }
    }
}

/// E20: the venues' price pass: `× (1 + venue_wage_pass × (mean wage_rev − 1))`.
pub fn venue_price_mult(world: &World) -> f32 {
    if !on(world) {
        return 1.0;
    }
    1.0 + world.config.economy2.venue_wage_pass * (mean_wage_rev(world) - 1.0)
}

/// E45: open vacancies city-wide.
pub fn vacancies_open(world: &World) -> u32 {
    world.vacancies.values().map(|v| v.len() as u32).sum()
}
