//! Life pass L2 § 1 (plan L10): the Treasury budget band. Daily at
//! midnight (`living::run`, after `corps::daily`, so `upkeep_mult` applies
//! to the next midnight's upkeep): above `hi` for `band_hold_days` in a row
//! the city posts public-works Sanitation jobs at the Recycler, and once
//! they stand full lowers corp upkeep; below `lo` it raises upkeep back and
//! lays off the newest public-works hire. The dole is never touched.
//! P10: the band runs under the levy (`daily`); under the market wage law
//! the tax band reads the Treasury and `market_works` runs public works on
//! its reading.

use crate::components::{BuildingKind, Job, Role};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::world::World;

/// Plan L10: corp upkeep × `upkeep_mult` (only with the band on).
pub fn upkeep_cost(world: &World, cost: i64) -> i64 {
    (cost as f32 * world.budget.upkeep_mult).round() as i64
}

/// Plan L10: a hire into a public-works vacancy joins `jobs_book.works`
/// (`demography::job_search`).
pub fn note_hire(world: &mut World, id: EntityId, employer: EntityId, role: Role) {
    if role != Role::Sanitation || world.works_vacancies == 0 {
        return;
    }
    if world.building_of_kind(BuildingKind::Cemetery) != Some(employer) {
        return;
    }
    world.works_vacancies -= 1;
    world.jobs_book.works.push(id);
    // L2 phase 5: public works pay `[budget] works_wage` when set.
    let wage = world.config.budget.works_wage;
    if wage > 0 {
        if let Some(j) = world.comp_mut::<Job>(id) {
            j.wage_per_day = wage;
        }
    }
}

/// Open public-works vacancies withdrawn (below the band, or the lever off).
fn withdraw_vacancies(world: &mut World, recycler: EntityId) {
    let mut left = world.works_vacancies;
    if let Some(v) = world.vacancies.get_mut(&recycler) {
        while left > 0 {
            let Some(i) = v.iter().rposition(|&r| r == Role::Sanitation) else { break };
            v.remove(i);
            left -= 1;
        }
        if v.is_empty() {
            world.vacancies.remove(&recycler);
        }
    }
    world.works_vacancies = 0;
}

/// The works roster: hires still at the Recycler as Sanitation; a vacancy
/// someone else filled (a non-works path) is no longer open.
fn refresh_roster(world: &mut World, recycler: EntityId) {
    let works: Vec<EntityId> = world
        .jobs_book
        .works
        .iter()
        .copied()
        .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == Role::Sanitation && j.employer == Some(recycler)))
        .collect();
    world.jobs_book.works = works;
    let open = world.vacancies.get(&recycler).map_or(0, |v| v.iter().filter(|&&r| r == Role::Sanitation).count());
    world.works_vacancies = world.works_vacancies.min(u16::try_from(open).unwrap_or(u16::MAX));
}

/// Post `works_step` public-works vacancies (up to `works_max` standing)
/// while the lever is on.
fn post_works(world: &mut World, recycler: EntityId, posted: u16, t: i64) {
    let (works_max, works_step) = (world.config.budget.works_max, world.config.budget.works_step);
    if world.levers.public_works && posted < works_max {
        let n = works_step.min(works_max - posted);
        world.vacancies.entry(recycler).or_default().extend(std::iter::repeat_n(Role::Sanitation, n as usize));
        world.works_vacancies += n;
        world.push_event(
            EventKind::WorksPosted,
            &[recycler],
            format!("the city posted {n} public-works jobs (Treasury {t}, {} standing)", posted + n),
        );
    }
}

/// Fix round (c): the open works vacancies withdrawn plus `works_step`
/// hires laid off, newest first (was one a day).
fn lay_off_works(world: &mut World, recycler: EntityId, t: i64) {
    if world.works_vacancies > 0 {
        withdraw_vacancies(world, recycler);
    }
    for _ in 0..world.config.budget.works_step {
        let Some(newest) = world.jobs_book.works.pop() else { break };
        let text = format!("{} laid off from public works (Treasury {t})", world.name_of(newest));
        crate::systems::economy::dismiss(world, newest, Some(recycler), text);
        world.sweep_beats.remove(&newest);
    }
}

/// The midnight pass (spec § 1, plan L10), under the levy (P10).
pub fn daily(world: &mut World) {
    let Some(recycler) = world.building_of_kind(BuildingKind::Cemetery) else { return };
    refresh_roster(world, recycler);

    let cfg = world.config.budget.clone();
    let t = world.treasury().map_or(0, |x| x.coins);
    let side: i8 = if t > cfg.band[1] {
        1
    } else if t < cfg.band[0] {
        -1
    } else {
        0
    };
    if side != 0 && side == world.budget.band_side {
        world.budget.band_days = world.budget.band_days.saturating_add(1);
    } else {
        world.budget.band_side = side;
        world.budget.band_days = u16::from(side != 0);
    }
    // Fix round (a): every step, cut or restore, waits `band_hold_days`
    // on its side (hysteresis on each step, not only on entry): a step
    // fires on the hold's multiples. Before: a 0.05 cut every day while
    // above `hi` (15 cuts before the Treasury crossed back, then 14 days of
    // single restores; seed 42's Treasury 76k -> 63 coins on day 61).
    let hold = cfg.band_hold_days.max(1);
    let held = world.budget.band_days >= hold && world.budget.band_days.is_multiple_of(hold);
    // Fix round (b): the forward guard: no upkeep cut while the Treasury
    // is under `hi` plus a week of yesterday's dole bill. `history.back()`
    // is yesterday: `stats::run` rolls the row at 23:59, this pass runs at
    // 00:00 (`current` is the new day, its dole still 0).
    let dole = world.stats.history.back().map_or(0, |r| r.flow_dole);
    let cut_ok = t >= cfg.band[1] + 7 * dole;
    if !world.levers.public_works && world.works_vacancies > 0 {
        withdraw_vacancies(world, recycler);
    }
    let posted = world.jobs_book.works.len() as u16 + world.works_vacancies;
    if side == 1 && held {
        post_works(world, recycler, posted, t);
        // The works stand full (deviation: within one `works_step` of
        // `works_max`, as hires quit and are replaced daily) or the lever is
        // off: corp upkeep falls.
        let hired = world.jobs_book.works.len() as u16;
        if cut_ok && (!world.levers.public_works || hired + cfg.works_step > cfg.works_max) {
            world.budget.upkeep_mult = (world.budget.upkeep_mult - cfg.upkeep_step).max(cfg.upkeep_floor);
        }
    } else if side == -1 && held {
        // Fix round (c): the restore is asymmetric: twice the cut's step,
        // and the works laid off.
        world.budget.upkeep_mult = (world.budget.upkeep_mult + 2.0 * cfg.upkeep_step).min(1.0);
        lay_off_works(world, recycler, t);
    }
    world.budget.works_posted = world.jobs_book.works.len() as u16 + world.works_vacancies;
    // Tidy the f32 so the CSV and saves read 0.95, not 0.9499999.
    world.budget.upkeep_mult = (world.budget.upkeep_mult * 100.0).round() / 100.0;
}

/// P10 (docs/P10_POLICY.md): public works under the market wage law. The
/// Treasury's tax band (`treasury::daily`, just before) has read the side
/// and the days on it; on the hold's multiples above its top the city
/// posts `works_step` posts while the lever is on, below its bottom it
/// withdraws the open ones and lays off `works_step` hires, newest first
/// (L2's rule without the levy cut: there is no levy to cut). A city that
/// never enacted public works has no roster and returns at once.
pub fn market_works(world: &mut World) {
    if !world.levers.public_works && world.jobs_book.works.is_empty() && world.works_vacancies == 0 {
        return;
    }
    let Some(recycler) = world.building_of_kind(BuildingKind::Cemetery) else { return };
    refresh_roster(world, recycler);
    let t = world.treasury().map_or(0, |x| x.coins);
    let hold = world.config.budget.band_hold_days.max(1);
    let (side, days) = (world.budget.band_side, world.budget.band_days);
    let held = days >= hold && days.is_multiple_of(hold);
    if !world.levers.public_works && world.works_vacancies > 0 {
        withdraw_vacancies(world, recycler);
    }
    let posted = world.jobs_book.works.len() as u16 + world.works_vacancies;
    if side == 1 && held {
        post_works(world, recycler, posted, t);
    } else if side == -1 && held {
        lay_off_works(world, recycler, t);
    }
    world.budget.works_posted = world.jobs_book.works.len() as u16 + world.works_vacancies;
}
