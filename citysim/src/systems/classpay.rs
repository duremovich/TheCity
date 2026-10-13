//! Class pay (docs/CLASS_PAY.md): graded posts and wages. A Job's grade (0
//! labour, 1 skilled, 2 professional, 3 management) multiplies its role's
//! wage (`[classpay] grade_mult`), fixed in `Job.wage_per_day` at hire or
//! promotion so both tiers pay it through `economy::collect_wage_scaled`.
//! Grades apply only at corp-owned workplaces, whose payroll the revenue
//! rule already holds at `labour_share × R` (`wages`): a graded post is a
//! bigger share of the same pool, never new money. Hiring into a graded
//! post (>= 2) is by merit (education, skill, tenure), from the jobless and
//! from lower-graded posts; the skilled grade is a promotion in place. An
//! adult's background (`Schooling`) is a pure hash of its origin class.
//! Nothing here runs without [`on`] (the market wage law and `[classpay]
//! enabled`): the dole city is byte-identical.

use crate::components::{Building, BuildingKind, Corp, Household, Job, Role, Schooling, Skills};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership;
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// The pure-hash purposes (no RNG stream is drawn).
const HASH_EDU: u64 = 0xc1a5_0001;
const HASH_CHILD: u64 = 0xc1a5_0002;

/// The professional grade: posts at or above it are hired by merit.
pub const PROFESSIONAL: u8 = 2;
/// The management grade.
pub const MANAGEMENT: u8 = 3;

/// The market wage law (P10: the levy not enacted) and `[classpay] enabled`.
pub fn on(world: &World) -> bool {
    crate::systems::wages::on(world) && world.config.classpay.enabled
}

/// Class pay: does this `[[trades]]` row post places in this city (a
/// `classpay` row only with class pay on)?
pub fn trade_live(world: &World, row: &crate::config::TradeCfg) -> bool {
    (!row.classpay || on(world)) && (!row.jobs2 || crate::systems::industry::on(world))
}

/// Is `b` a standing-or-not building owned by a corp?
fn corp_owned(world: &World, b: Option<EntityId>) -> bool {
    b.is_some_and(|b| world.corp_of_building(b).is_some())
}

/// The base grade of a `role` post at `employer`: a trade row's `grade`, a
/// bespoke role's `role_grade` (labour when unlisted). 0 with class pay
/// off or at a workplace no corp owns.
pub fn base_grade(world: &World, employer: Option<EntityId>, role: Role) -> u8 {
    if !on(world) || !corp_owned(world, employer) {
        return 0;
    }
    role_grade(world, role)
}

/// The base grade of `role` wherever a corp employs it (no owner check).
pub fn role_grade(world: &World, role: Role) -> u8 {
    let g = match role {
        Role::Trade(t) => world.config.trade(t).map_or(0, |r| r.grade),
        _ => world.config.classpay.role_grade.get(&format!("{role:?}")).copied().unwrap_or(0),
    };
    g.min(MANAGEMENT)
}

/// The wage multiple of `grade`.
pub fn mult(world: &World, grade: u8) -> f32 {
    world.config.classpay.grade_mult.get(usize::from(grade.min(MANAGEMENT))).copied().unwrap_or(1.0)
}

/// The daily wage of a `role` post at `grade` (the config wage at grade 0
/// or class pay off, exactly).
pub fn wage_at(world: &World, role: Role, grade: u8) -> i64 {
    let base = world.config.wage(role);
    if grade == 0 || !on(world) {
        return base;
    }
    (base as f32 * mult(world, grade)).round() as i64
}

/// The marginal wage the margin rule prices a new `role` place at
/// `employer` at (before `wage_rev`): the config wage times the post's base
/// grade multiple; the config wage with class pay off.
pub fn post_wage(world: &World, employer: EntityId, role: Role) -> f32 {
    let base = world.config.wage(role) as f32;
    let g = base_grade(world, Some(employer), role);
    if g == 0 {
        base
    } else {
        base * mult(world, g)
    }
}

/// A Job's title: "Senior <label>" at the skilled grade, else the label.
pub fn title(job: &Job) -> String {
    if job.grade == 1 {
        format!("Senior {}", job.role.label())
    } else {
        job.role.label().to_string()
    }
}

// ---------------------------------------------------------------------------
// Background (Schooling)
// ---------------------------------------------------------------------------

/// The class of a Home by tier: 1 Sump, 2 Mid, 3 Spire; 0 no Home (a Dreg).
pub fn home_class(world: &World, home: Option<EntityId>) -> u8 {
    home.and_then(|h| world.comp::<Building>(h)).map_or(0, |b| b.tier.min(2) + 1)
}

/// An adult's origin class and education: its `Schooling`, else (a body
/// made before class pay was on) the origin of its Home now and that
/// origin's mean education.
pub fn schooling(world: &World, id: EntityId) -> Schooling {
    if let Some(s) = world.comp::<Schooling>(id) {
        return *s;
    }
    let origin = home_class(world, world.comp::<Household>(id).and_then(|h| h.home));
    Schooling { origin, education: mean_education(world, origin) }
}

fn mean_education(world: &World, origin: u8) -> f32 {
    world.config.classpay.education.get(usize::from(origin.min(3))).copied().unwrap_or(0.0)
}

/// A seeded or arriving adult's background: its Home's class and the
/// class's mean education ± `education_spread` (a pure hash of the seed and
/// the agent).
fn seeded(world: &World, id: EntityId, home: Option<EntityId>) -> Schooling {
    seeded_keyed(world, u64::from(id.index), home)
}

fn seeded_keyed(world: &World, key: u64, home: Option<EntityId>) -> Schooling {
    let origin = home_class(world, home);
    let u = crate::systems::econ::hash_unit(world.seed(), HASH_EDU, key, 0);
    let spread = world.config.classpay.education_spread;
    let education = (mean_education(world, origin) + (2.0 * u - 1.0) * spread).clamp(0.0, 1.0);
    Schooling { origin, education }
}

/// World creation: every citizen's background from its Home (no RNG).
pub fn seed_schooling(world: &mut World) {
    if !on(world) {
        return;
    }
    for id in world.citizens() {
        let home = world.comp::<Household>(id).and_then(|h| h.home);
        let s = seeded(world, id, home);
        world.insert(id, s);
    }
}

/// An immigrant's background, from its arrival Home.
pub fn on_arrival(world: &mut World, id: EntityId, home: Option<EntityId>) {
    on_arrival_keyed(world, id, home, u64::from(id.index));
}

/// [`on_arrival`] with the education hash keyed on `key` (Scale S5: a
/// materialised Ambient adult's flat).
pub fn on_arrival_keyed(world: &mut World, id: EntityId, home: Option<EntityId>, key: u64) {
    if !on(world) {
        return;
    }
    let s = seeded_keyed(world, key, home);
    world.insert(id, s);
}

/// A newborn's background: its household's class as origin, its parents'
/// mean education ± `child_spread`.
pub fn on_birth(world: &mut World, id: EntityId, mother: EntityId, father: EntityId, home: EntityId) {
    if !on(world) {
        return;
    }
    let origin = home_class(world, Some(home));
    let (m, f) = (schooling(world, mother).education, schooling(world, father).education);
    let u = crate::systems::econ::hash_unit(world.seed(), HASH_CHILD, u64::from(id.index), 0);
    let education = ((m + f) / 2.0 + (2.0 * u - 1.0) * world.config.classpay.child_spread).clamp(0.0, 1.0);
    world.insert(id, Schooling { origin, education });
}

// ---------------------------------------------------------------------------
// Merit
// ---------------------------------------------------------------------------

/// The skill a `role` post ranks on: the role's competence skill, else the
/// mean of knowledge and persuasion (an office post).
fn post_skill(world: &World, id: EntityId, role: Role) -> f32 {
    crate::systems::competence::role_skill(world, id, role)
        .map(|(v, _, _)| v)
        .or_else(|| world.comp::<Skills>(id).map(|s| 0.5 * (s.knowledge + s.persuasion)))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

/// Days `id` has held its current Job (0 without one).
fn tenure_days(world: &World, id: EntityId) -> u64 {
    world.comp::<Job>(id).map_or(0, |j| world.tick.saturating_sub(j.hired_tick) / TICKS_PER_DAY)
}

/// `id`'s merit for a `role` post at `employer`: `merit_edu × education +
/// merit_skill × skill + merit_tenure × tenure` (tenure counted only at the
/// employer's own corp, over `tenure_full_days`, capped at 1), minus
/// `commute_w ×` the tiles from its Home (else where it stands) to the door.
pub fn merit(world: &World, id: EntityId, employer: EntityId, role: Role) -> f32 {
    let cfg = &world.config.classpay;
    let edu = schooling(world, id).education;
    let skill = post_skill(world, id, role);
    let corp = world.corp_of_building(employer);
    let same = corp.is_some()
        && world.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e)) == corp;
    let tenure = if same { (tenure_days(world, id) as f32 / cfg.tenure_full_days.max(1) as f32).min(1.0) } else { 0.0 };
    let door = world.comp::<Building>(employer).map(|b| b.door);
    let from = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<crate::components::Position>(id).map(|p| p.tile));
    let tiles = match (from, door) {
        (Some(a), Some(b)) => a.manhattan(b) as f32,
        _ => 0.0,
    };
    cfg.merit_edu * edu + cfg.merit_skill * skill + cfg.merit_tenure * tenure - cfg.commute_w * tiles
}

/// Graded hiring (grade >= 2): the candidate a `role` vacancy at `employer`
/// goes to: the highest merit (ties the lower id) among the free adults who
/// are jobless or hold a lower-graded post they have held `promote_days`
/// (waived at world creation), not an exec, not leaving, not blocked from
/// this employer by a quit. `None` when the post is not graded.
pub fn pick_graded(world: &World, employer: EntityId, role: Role) -> Option<EntityId> {
    let grade = base_grade(world, Some(employer), role);
    if grade < PROFESSIONAL {
        return None;
    }
    let seeding = world.tick == 0;
    let promote = u64::from(world.config.classpay.promote_days);
    let execs = crate::systems::classes::exec_set(world);
    let mut best: Option<(f32, EntityId)> = None;
    for id in world.citizens() {
        if !crate::systems::demography::is_adult(world, id)
            || execs.contains(&id)
            || world.has::<crate::components::Sentence>(id)
            || world.comp::<crate::components::Brain>(id).is_none_or(|b| b.emigrating)
            || crate::systems::life::quit_blocks(world, id, employer)
        {
            continue;
        }
        if let Some(j) = world.comp::<Job>(id) {
            if j.grade >= grade || (j.employer == Some(employer) && j.role == role) {
                continue;
            }
            if !seeding && tenure_days(world, id) < promote {
                continue;
            }
        }
        let m = merit(world, id, employer, role);
        if best.is_none_or(|(bm, bid)| m > bm || (m == bm && id < bid)) {
            best = Some((m, id));
        }
    }
    best.map(|(_, id)| id)
}

/// A graded hire who holds a post: it leaves it for the new one (vacated
/// and re-posted as a quit's would be, no Quit event). Returns whether it held one.
pub fn leave_for_promotion(world: &mut World, id: EntityId) -> bool {
    if !world.has::<Job>(id) {
        return false;
    }
    world.abort_plan(id);
    world.vacate_job(id);
    true
}

/// `Promoted [id, workplace]`.
pub fn note_promoted(world: &mut World, id: EntityId, workplace: Option<EntityId>) {
    let name = world.name_of(id);
    let what = world.comp::<Job>(id).map_or_else(|| "exec".to_string(), title);
    let at = workplace.map(|b| world.name_of(b)).unwrap_or_default();
    let mut actors = vec![id];
    actors.extend(workplace);
    world.push_event(EventKind::Promoted, &actors, format!("{name} promoted to {what} at {at}"));
}

// ---------------------------------------------------------------------------
// The skilled grade (promotion in place)
// ---------------------------------------------------------------------------

/// Midnight (`living::run`, after `wages::staff`): the skilled promotions.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    night_school(world);
    promote_skilled(world, false);
}

/// P3 (Dylan 2026-10-10, the thin ladder): a day held in a skilled or
/// graded post is a day of night school: `education += edu_gain_per_day`,
/// up to `edu_cap` (an education already above the cap is kept). A
/// hard-working Sump-born worker can reach a professional post in a few sim
/// years: rare, possible, and deterministic (no draw).
pub fn night_school(world: &mut World) {
    let (gain, cap) = (world.config.classpay.edu_gain_per_day, world.config.classpay.edu_cap);
    if gain <= 0.0 {
        return;
    }
    for id in world.with::<Job>() {
        if world.comp::<Job>(id).is_none_or(|j| j.grade == 0) {
            continue;
        }
        let s = schooling(world, id);
        if s.education >= cap {
            continue;
        }
        let education = (s.education + gain).min(cap);
        world.insert(id, Schooling { origin: s.origin, education });
    }
}

/// Per standing corp workplace and labour role there: `floor(staff ×
/// skilled_share)` skilled posts; each open one goes to the role's best
/// labour holder at the building by merit (no commute term: it already
/// works there) with `promote_days` of tenure (`seeding`: waived, no
/// event). Returns the promotions.
pub fn promote_skilled(world: &mut World, seeding: bool) -> u32 {
    let share = world.config.classpay.skilled_share;
    if share <= 0.0 {
        return 0;
    }
    let promote = u64::from(world.config.classpay.promote_days);
    let mut done = 0;
    for corp in world.corps() {
        let Some(c) = world.comp::<Corp>(corp) else { continue };
        let (buildings, exec) = (c.buildings.clone(), c.exec);
        for b in buildings {
            if world.comp::<Building>(b).is_none_or(|bd| bd.demolished || bd.derelict || bd.kind == BuildingKind::Lot) {
                continue;
            }
            let staff: Vec<EntityId> = ownership::staff_at(world, b).into_iter().filter(|&a| Some(a) != exec).collect();
            let mut roles: Vec<Role> = staff.iter().filter_map(|&a| world.comp::<Job>(a).map(|j| j.role)).collect();
            roles.sort();
            roles.dedup();
            for role in roles {
                if role_grade(world, role) != 0 {
                    continue;
                }
                let holders: Vec<(u8, EntityId)> = staff
                    .iter()
                    .filter_map(|&a| world.comp::<Job>(a).filter(|j| j.role == role).map(|j| (j.grade, a)))
                    .collect();
                let slots = (holders.len() as f32 * share).floor() as usize;
                let have = holders.iter().filter(|&&(g, _)| g >= 1).count();
                let mut pool: Vec<(f32, EntityId)> = holders
                    .iter()
                    .filter(|&&(g, a)| g == 0 && (seeding || tenure_days(world, a) >= promote))
                    .map(|&(_, a)| {
                        let cfg = &world.config.classpay;
                        let tenure = (tenure_days(world, a) as f32 / cfg.tenure_full_days.max(1) as f32).min(1.0);
                        let m = cfg.merit_edu * schooling(world, a).education
                            + cfg.merit_skill * post_skill(world, a, role)
                            + cfg.merit_tenure * tenure;
                        (m, a)
                    })
                    .collect();
                // Best first (ties the lower id).
                pool.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
                for (_, a) in pool.into_iter().take(slots.saturating_sub(have)) {
                    let wage = wage_at(world, role, 1);
                    if let Some(j) = world.comp_mut::<Job>(a) {
                        j.grade = 1;
                        j.wage_per_day = wage;
                    }
                    if !seeding {
                        note_promoted(world, a, Some(b));
                    }
                    done += 1;
                }
            }
        }
    }
    done
}

// ---------------------------------------------------------------------------
// Seeding, layoffs, execs, housing
// ---------------------------------------------------------------------------

/// World creation, after `wages::seed_staff` (whose job search filled the
/// graded posts first by merit): a second job search for the posts the
/// graded hires left, then the skilled grade by merit (tenure waived).
pub fn seed_after_staff(world: &mut World) {
    if !on(world) {
        return;
    }
    crate::systems::demography::job_search(world);
    promote_skilled(world, true);
}

/// Is `id` held back from a layoff (a professional or management post)?
pub fn protected(world: &World, id: EntityId) -> bool {
    on(world) && world.comp::<Job>(id).is_some_and(|j| j.grade >= PROFESSIONAL)
}

/// Does `id` keep its Home's tier when it relocates (a graded post holder
/// or an exec)? Returns the lowest tier it moves to (0: any).
pub fn min_home_tier(world: &World, id: EntityId, home: EntityId) -> u8 {
    if !on(world) {
        return 0;
    }
    let anchored = world.comp::<Job>(id).is_some_and(|j| j.grade >= PROFESSIONAL)
        || world.corps().into_iter().any(|c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(id)));
    if !anchored {
        return 0;
    }
    world.comp::<Building>(home).map_or(0, |b| b.tier)
}

/// A replacement exec among the corp's management, then professional, post
/// holders (L1's `pick_exec` among them); `None` to fall back to the rule
/// without class pay.
pub fn exec_from_ranks(world: &World, corp: EntityId) -> Option<EntityId> {
    if !on(world) {
        return None;
    }
    let execs = crate::systems::classes::exec_set(world);
    let staff = ownership::employees_of(world, corp);
    for floor in [MANAGEMENT, PROFESSIONAL] {
        let ranks = staff
            .iter()
            .copied()
            .filter(|a| !execs.contains(a))
            .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.grade >= floor));
        if let Some(e) = crate::systems::life::pick_exec(world, ranks) {
            return Some(e);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The graded premium out of capital and profit (P3, Dylan 2026-10-10)
// ---------------------------------------------------------------------------

/// The premium part of a `gross` graded wage: what it pays above the labour
/// rate (`gross − gross ÷ grade_mult`); 0 for labour or with class pay off.
pub fn premium_of(world: &World, job: &Job, gross: i64) -> i64 {
    if job.grade == 0 || gross <= 0 || !on(world) {
        return 0;
    }
    let m = mult(world, job.grade).max(1.0);
    (gross - (gross as f32 / m).round() as i64).max(0)
}

/// A graded `role` place's premium a day at `employer`: the role's wage ×
/// (multiple − 1) × the corp's wage multipliers.
pub fn marginal_premium(world: &World, employer: EntityId, role: Role) -> f32 {
    let g = base_grade(world, Some(employer), role);
    if g == 0 {
        return 0.0;
    }
    let rev =
        world.corp_of_building(employer).and_then(|c| world.comp::<Corp>(c)).map_or(1.0, |c| c.wage_mult * c.wage_rev);
    world.config.wage(role) as f32 * (mult(world, g) - 1.0) * rev
}

fn ring_mean(r: &std::collections::VecDeque<i64>) -> f32 {
    if r.is_empty() {
        0.0
    } else {
        r.iter().sum::<i64>() as f32 / r.len() as f32
    }
}

/// `(premium a day, profit before it)` of a corp: the premium window's mean
/// and the cashflow window's mean with the premium added back.
fn premium_and_profit(c: &Corp) -> (f32, f32) {
    let prem = ring_mean(&c.prem);
    (prem, (ring_mean(&c.cashflow) + prem).max(0.0))
}

/// The coins a day a corp's capital and profit leave for more graded
/// premium: `profit + premium_payout_share × capital payout − premium`
/// (`None` before the windows
/// fill, or with class pay off: no limit; the seeded posts are paid from
/// the capital `seed_staff` priced them against).
pub fn premium_room(world: &World, corp: EntityId) -> Option<f32> {
    if !on(world) {
        return None;
    }
    let (r, p) = crate::systems::wages::windows(world, corp)?;
    let (prem, profit) = premium_and_profit(world.comp::<Corp>(corp)?);
    let payout = crate::systems::wages::capital_payout(world, corp, r, p);
    // P3 (Dylan): the premium may draw on at most `premium_payout_share` of
    // the payout; the rest is always labour's.
    let share = world.config.classpay.premium_payout_share.clamp(0.0, 1.0);
    Some(profit + share * payout - prem)
}

/// Can the corp owning `employer` fund one more `role` place's premium?
/// True for a labour post, a non-corp employer, or before the windows fill.
pub fn premium_funded(world: &World, employer: EntityId, role: Role) -> bool {
    let extra = marginal_premium(world, employer, role);
    if extra <= 0.0 {
        return true;
    }
    let Some(corp) = world.corp_of_building(employer) else { return true };
    premium_room(world, corp).is_none_or(|room| room >= extra)
}

/// The capital payout left to the labour pool's target: the premium is
/// charged to profit first, then to at most `premium_payout_share` of the
/// payout (`payout` with class pay off).
pub fn labour_payout(world: &World, corp: EntityId, payout: f32) -> f32 {
    if !on(world) {
        return payout;
    }
    let Some(c) = world.comp::<Corp>(corp) else { return payout };
    let (prem, profit) = premium_and_profit(c);
    // P3 (Dylan): at most `premium_payout_share` of the payout goes to the
    // premium; the rest always reaches labour's P*.
    let share = world.config.classpay.premium_payout_share.clamp(0.0, 1.0);
    let drawn = (prem - profit).max(0.0).min(share * payout.max(0.0));
    (payout - drawn).max(0.0)
}

/// Midnight (`wages::staff`): a corp that cannot fund its graded premium
/// withdraws its open graded vacancies (graded hiring stops; nobody is laid
/// off for it). Returns the vacancies withdrawn.
pub fn withdraw_unfunded(world: &mut World, corp: EntityId) -> u32 {
    let Some(room) = premium_room(world, corp) else { return 0 };
    if room > 0.0 {
        return 0;
    }
    let buildings = world.comp::<Corp>(corp).map(|c| c.buildings.clone()).unwrap_or_default();
    let mut n = 0;
    for b in buildings {
        let graded: Vec<Role> = world
            .vacancies
            .get(&b)
            .map(|v| v.iter().copied().filter(|&r| base_grade(world, Some(b), r) >= PROFESSIONAL).collect())
            .unwrap_or_default();
        if graded.is_empty() {
            continue;
        }
        if let Some(v) = world.vacancies.get_mut(&b) {
            let before = v.len();
            v.retain(|r| !graded.contains(r));
            n += (before - v.len()) as u32;
            if v.is_empty() {
                world.vacancies.remove(&b);
            }
        }
    }
    n
}

// ---------------------------------------------------------------------------
// The status sink (P3, Dylan 2026-10-10)
// ---------------------------------------------------------------------------

/// The nearest standing, owned building to `from` among `kinds` that `pred`
/// accepts (door Manhattan, ties the lower id).
fn nearest_owned(
    world: &World,
    kinds: &[BuildingKind],
    from: crate::components::TilePos,
    pred: impl Fn(&Building) -> bool,
) -> Option<EntityId> {
    kinds
        .iter()
        .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
        .filter_map(|b| {
            let bd = world.comp::<Building>(b)?;
            (!bd.demolished && !bd.derelict && bd.owner.is_some() && pred(bd)).then(|| (bd.door.manhattan(from), b))
        })
        .min()
        .map(|(_, b)| b)
}

/// Midnight (`living::run`, after the promotions), both tiers alike (a
/// ledger, as rent): an adult holding more than `status_floor` coins spends
/// `status_frac` of the excess a day on status, split by `status_split`:
/// a night out at the Spire (the nearest Lounge, else a Spire Club; paid as
/// a visit, `Flow::Leisure`, with the venue's fun), fashion at the nearest
/// corp Market and cosmetic chrome at the nearest corp Clinic (each an
/// `Flow::Asset` sale to its owner). Every coin reaches an owner's purse and
/// its building's revenue, so it pays the venues' and the shops' staff
/// through the wage rule. Returns the coins spent.
pub fn status_daily(world: &mut World) -> i64 {
    if !on(world) {
        return 0;
    }
    let cfg = world.config.classpay.clone();
    if cfg.status_frac <= 0.0 {
        return 0;
    }
    let mut spent = 0;
    for id in world.citizens() {
        if !crate::systems::demography::is_adult(world, id)
            || world.has::<crate::components::Sentence>(id)
            || world.comp::<crate::components::Brain>(id).is_none_or(|b| b.emigrating)
        {
            continue;
        }
        let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
        if coins <= cfg.status_floor {
            continue;
        }
        let budget = (cfg.status_frac * (coins - cfg.status_floor) as f32).floor() as i64;
        if budget <= 0 {
            continue;
        }
        let Some(from) = world
            .comp::<Household>(id)
            .and_then(|h| h.home)
            .and_then(|h| world.comp::<Building>(h))
            .map(|b| b.door)
            .or_else(|| world.comp::<crate::components::Position>(id).map(|p| p.tile))
        else {
            continue;
        };
        let shares = cfg.status_split;
        let total: f32 = shares.iter().sum::<f32>().max(1e-6);
        let part = |i: usize| (budget as f32 * shares[i] / total).floor() as i64;
        // The night out.
        let venue = nearest_owned(world, &[BuildingKind::Lounge], from, |b| b.venue.is_some())
            .or_else(|| nearest_owned(world, &[BuildingKind::Club], from, |b| b.venue.is_some() && b.tier >= 2));
        if let Some(v) = venue {
            let amount = part(0);
            if amount > 0 && crate::systems::leisure::door_ok(world, id, v) {
                let paid = crate::systems::leisure::pay_venue(world, id, v, amount, ownership::Flow::Leisure);
                if paid > 0 {
                    let fun = crate::systems::leisure::gain_at(world, v);
                    if let Some(n) = world.comp_mut::<crate::components::Needs>(id) {
                        n.fun = (n.fun + fun).min(1.0);
                    }
                    spent += paid;
                }
            }
        }
        // Fashion and cosmetic chrome, from the corps' shops.
        let corp_owned = |world: &World, b: &Building| b.owner.is_some_and(|o| world.has::<Corp>(o));
        for (i, kind) in [(1usize, BuildingKind::Market), (2, BuildingKind::Clinic)] {
            let amount = part(i);
            if amount <= 0 {
                continue;
            }
            let Some(shop) = nearest_owned(world, &[kind], from, |b| corp_owned(world, b)) else { continue };
            let owner = world.owner_of(shop);
            let paid = ownership::pay(world, Some(id), owner, amount, ownership::Flow::Asset);
            if paid > 0 {
                ownership::credit(world, shop, paid);
                spent += paid;
            }
        }
    }
    world.probe.status_spent += spent;
    spent
}
