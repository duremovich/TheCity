//! Class-tiered jobs and pay (docs/CLASS_PAY.md).
//!
//! `class_table` (`#[ignore]`, a printed finding, never a band): seeds
//! 42-44 x 120 days in threads, the classes by Home tier (Spire, Mid, Sump,
//! and the Dregs: no Home) at days 0, 30, 60, 90 and 119: adults, employed
//! share, mean coins, mean gross wage of the employed, the synth share of
//! the last 30 days' meals; the city's wallet Gini, employed, corps alive,
//! population and Treasury. Run it on the flipped config:
//! `CITYSIM_ASSETS=<flipped copy> cargo test --release -p citysim --test
//! classpay -- --ignored class_table --nocapture`.

use std::collections::BTreeMap;

use citysim::systems::{demography, wages};
use citysim::{Building, Config, Health, Household, Job, Wallet, World, TICKS_PER_DAY};

/// The class of an adult by where it sleeps: 3 Spire, 2 Mid, 1 Sump (the
/// Home's tier + 1), 0 a Dreg (no Home).
fn home_class(w: &World, id: citysim::EntityId) -> usize {
    // `CLASSPAY_BY=origin`: by the background class (`Schooling.origin`).
    if std::env::var("CLASSPAY_BY").is_ok_and(|v| v == "origin") {
        if let Some(s) = w.comp::<citysim::Schooling>(id) {
            return usize::from(s.origin.min(3));
        }
    }
    w.comp::<Household>(id)
        .and_then(|h| h.home)
        .and_then(|h| w.comp::<Building>(h))
        .map_or(0, |b| usize::from(b.tier.min(2)) + 1)
}

const CLASS: [&str; 4] = ["Dreg", "Sump", "Mid", "Spire"];

#[derive(Default, Clone)]
struct Acc {
    n: u32,
    employed: u32,
    coins: i64,
    wage: i64,
    /// (synthy meals, all meals) of the window.
    synth: u32,
    meals: u32,
    /// Job holders by grade.
    grades: [u32; 4],
    /// Every wallet (for the median).
    wallets: Vec<i64>,
    /// Labour-grade (0) holders and their gross wages.
    lab_n: u32,
    lab_wage: i64,
}

fn median(v: &[i64]) -> i64 {
    let mut v = v.to_vec();
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or(0)
}

fn gini(mut v: Vec<i64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort();
    let n = v.len() as f64;
    let sum: f64 = v.iter().map(|&x| x.max(0) as f64).sum();
    if sum <= 0.0 {
        return 0.0;
    }
    let mut acc = 0.0;
    for (i, &x) in v.iter().enumerate() {
        acc += (2.0 * (i as f64 + 1.0) - n - 1.0) * x.max(0) as f64;
    }
    acc / (n * sum)
}

struct Snap {
    day: u64,
    by: Vec<Acc>,
    by_tier: BTreeMap<u8, (u32, i64)>,
    gini: f64,
    employed: u32,
    corps: usize,
    population: u32,
    treasury: i64,
    /// Coins the status sink spent so far (class pay P3).
    status: i64,
}

fn run_seed(seed: u64, days: u64) -> Vec<Snap> {
    let mut w = World::new(seed, Config::load().dole_city());
    let marks = [0u64, 30, 60, 90, 119, days - 1];
    // Meals by agent class over the window, read each day before midnight.
    let mut window: Vec<(u32, u32)> = vec![(0, 0); 4];
    let mut out = Vec::new();
    for day in 0..days {
        w.run_ticks(TICKS_PER_DAY - 1);
        for id in w.citizens() {
            if !demography::is_adult(&w, id) {
                continue;
            }
            let c = home_class(&w, id);
            if let Some(h) = w.comp::<Health>(id) {
                let t = &h.today;
                window[c].0 += u32::from(t.synth) + u32::from(t.alms);
                window[c].1 += u32::from(t.synth) + u32::from(t.alms) + u32::from(t.dive) + u32::from(t.real);
            }
        }
        if marks.contains(&day) {
            let mut by = vec![Acc::default(); 4];
            let mut coins = Vec::new();
            let mut by_tier: BTreeMap<u8, (u32, i64)> = BTreeMap::new();
            for id in w.citizens() {
                if !demography::is_adult(&w, id) {
                    continue;
                }
                let c = home_class(&w, id);
                let a = &mut by[c];
                a.n += 1;
                let k = w.comp::<Wallet>(id).map_or(0, |x| x.coins);
                a.coins += k;
                a.wallets.push(k);
                coins.push(k);
                if let Some(j) = w.comp::<Job>(id) {
                    a.employed += 1;
                    a.grades[usize::from(j.grade.min(3))] += 1;
                    if j.grade == 0 {
                        a.lab_n += 1;
                        a.lab_wage += wages::gross_wage(&w, j);
                    }
                    let g = wages::gross_wage(&w, j);
                    a.wage += g;
                    let e = by_tier.entry(j.grade).or_default();
                    e.0 += 1;
                    e.1 += g;
                }
            }
            for c in 0..4 {
                by[c].synth = window[c].0;
                by[c].meals = window[c].1;
            }
            window = vec![(0, 0); 4];
            out.push(Snap {
                day,
                by,
                by_tier,
                gini: gini(coins),
                employed: w.with::<Job>().len() as u32,
                corps: w.corps().len(),
                population: w.citizens().len() as u32,
                treasury: w.treasury().map_or(0, |t| t.coins),
                status: w.probe.status_spent,
            });
        }
        w.run_ticks(1);
    }
    out
}

#[test]
#[ignore = "tier: diag"]
fn class_table() {
    let days: u64 = std::env::var("CLASSPAY_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let handles: Vec<_> =
        [42u64, 43, 44].into_iter().map(|s| std::thread::spawn(move || (s, run_seed(s, days)))).collect();
    let runs: Vec<(u64, Vec<Snap>)> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for (seed, snaps) in &runs {
        println!("\n== seed {seed} ==");
        println!("day | class | adults | employed | mean coins | mean wage | synth% (window)");
        for s in snaps {
            for c in (0..4).rev() {
                let a = &s.by[c];
                let n = a.n.max(1) as f64;
                println!(
                    "{:>3} | {:<5} | {:>5} | {:>5.1}% | {:>8.1} | {:>5.2} | {:>5.1}%",
                    s.day,
                    CLASS[c],
                    a.n,
                    100.0 * a.employed as f64 / n,
                    a.coins as f64 / n,
                    a.wage as f64 / a.employed.max(1) as f64,
                    100.0 * a.synth as f64 / a.meals.max(1) as f64
                );
            }
            let tiers: Vec<String> = s
                .by_tier
                .iter()
                .map(|(g, (n, w))| format!("g{g}: {n} @ {:.2}", *w as f64 / (*n).max(1) as f64))
                .collect();
            println!(
                "{:>3} | city: gini {:.3}, employed {}, corps {}, population {}, treasury {} | {}",
                s.day,
                s.gini,
                s.employed,
                s.corps,
                s.population,
                s.treasury,
                tiers.join(", ")
            );
        }
    }
    // Pooled over the seeds: the class means at each mark.
    println!("\n== pooled 42-44 ==");
    println!("day | class | adults | employed | mean coins | median | mean wage | labour wage | synth%");
    let marks = runs[0].1.len();
    for m in 0..marks {
        for c in (0..4).rev() {
            let mut a = Acc::default();
            for (_, snaps) in &runs {
                let b = &snaps[m].by[c];
                a.n += b.n;
                a.employed += b.employed;
                a.coins += b.coins;
                a.wallets.extend_from_slice(&b.wallets);
                a.lab_n += b.lab_n;
                a.lab_wage += b.lab_wage;
                a.wage += b.wage;
                a.synth += b.synth;
                a.meals += b.meals;
                for g in 0..4 {
                    a.grades[g] += b.grades[g];
                }
            }
            let n = a.n.max(1) as f64;
            println!(
                "{:>3} | {:<5} | {:>5} | {:>5.1}% | {:>8.1} | {:>5} | {:>5.2} | {:>5.2} | {:>5.1}% | grades {:?}",
                runs[0].1[m].day,
                CLASS[c],
                a.n,
                100.0 * a.employed as f64 / n,
                a.coins as f64 / n,
                median(&a.wallets),
                a.wage as f64 / a.employed.max(1) as f64,
                a.lab_wage as f64 / a.lab_n.max(1) as f64,
                100.0 * a.synth as f64 / a.meals.max(1) as f64,
                a.grades
            );
        }
        let g: f64 = runs.iter().map(|(_, s)| s[m].gini).sum::<f64>() / runs.len() as f64;
        let e: Vec<String> = runs.iter().map(|(_, s)| s[m].employed.to_string()).collect();
        let p: Vec<String> = runs.iter().map(|(_, s)| s[m].population.to_string()).collect();
        let k: Vec<String> = runs.iter().map(|(_, s)| s[m].corps.to_string()).collect();
        let t: Vec<String> = runs.iter().map(|(_, s)| s[m].treasury.to_string()).collect();
        let st: Vec<String> = runs.iter().map(|(_, s)| s[m].status.to_string()).collect();
        println!(
            "{:>3} | status spent {} | gini {:.3} | employed {} | population {} | corps {} | treasury {}",
            runs[0].1[m].day,
            st.join("/"),
            g,
            e.join("/"),
            p.join("/"),
            k.join("/"),
            t.join("/")
        );
    }
}

// ---------------------------------------------------------------------------
// Mechanisms (unit tests)
// ---------------------------------------------------------------------------

use citysim::systems::{classpay, economy};
use citysim::{Brain, Corp, EntityId, Lod, Role, Schooling};

/// The class-pay city: the market wage law (the levy not enacted, P10) on
/// the dole city, class pay on as shipped.
fn config_on() -> Config {
    let mut cfg = Config::load().dole_city();
    cfg.policy.levy = false;
    assert!(cfg.classpay.enabled, "class pay ships enabled (read only with wages)");
    cfg
}

fn city_on() -> World {
    let w = World::new(42, config_on());
    assert!(classpay::on(&w));
    w
}

fn holders(w: &World, pred: impl Fn(&Job) -> bool) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = w.with::<Job>().into_iter().filter(|&a| w.comp::<Job>(a).is_some_and(&pred)).collect();
    v.sort();
    v
}

fn role(w: &World, key: &str) -> Role {
    w.config.trade_role(key).expect(key)
}

/// The default (dole) city with class pay's keys and rows in the config is
/// the city without them, byte for byte: every class-pay path reads the
/// market wage law (`wages::on`).
#[test]
fn test_default_city_identical_with_classpay_keys() {
    let a_cfg = Config::load().dole_city();
    assert!(a_cfg.policy.levy, "Config::dole_city: the levy (no wages from revenue)");
    assert!(a_cfg.trades.iter().any(|r| r.classpay), "the class-pay rows ship in the config");
    let mut b_cfg = a_cfg.clone();
    b_cfg.classpay.enabled = false;
    b_cfg.trades.retain(|r| !r.classpay);
    b_cfg.reset_trade_cache();
    let mut a = World::new(42, a_cfg);
    let mut b = World::new(42, b_cfg);
    for _ in 0..3 {
        a.run_ticks(TICKS_PER_DAY);
        b.run_ticks(TICKS_PER_DAY);
    }
    assert!(a.with::<Schooling>().is_empty(), "no backgrounds without wages");
    assert!(a.with::<Job>().into_iter().all(|j| a.comp::<Job>(j).is_some_and(|j| j.grade == 0)));
    b.config = a.config.clone();
    assert_eq!(citysim::save::to_ron(&a), citysim::save::to_ron(&b), "the dole city moved with class pay's keys");
}

/// A graded post's wage is fixed at hire: the role's wage times the grade's
/// multiple (a Manager 3x, a skilled post 1.3x); labour at the config wage
/// exactly.
#[test]
fn test_graded_wage_at_hire() {
    let w = city_on();
    let mult = w.config.classpay.grade_mult;
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager);
    assert!(!m.is_empty(), "the seeded city staffs Managers");
    for a in &m {
        let j = w.comp::<Job>(*a).expect("job");
        assert_eq!(j.grade, 3);
        assert_eq!(j.wage_per_day, (w.config.wage(manager) as f32 * mult[3]).round() as i64);
    }
    let skilled = holders(&w, |j| j.grade == 1);
    assert!(!skilled.is_empty(), "the seeded city has skilled posts");
    for a in &skilled {
        let j = w.comp::<Job>(*a).expect("job");
        assert_eq!(j.wage_per_day, (w.config.wage(j.role) as f32 * mult[1]).round() as i64, "{:?}", j.role);
    }
    for a in holders(&w, |j| j.grade == 0 && j.employer.is_some_and(|e| w.corp_of_building(e).is_some())) {
        let j = w.comp::<Job>(a).expect("job");
        assert_eq!(j.wage_per_day, w.config.wage(j.role), "{:?}", j.role);
    }
}

/// One pay path: a graded post pays its grade at the Full and at the
/// Statistical tier alike (`economy::collect_wage_scaled` reads the Job's
/// `wage_per_day`; the Statistical shift end calls it, P5f).
#[test]
fn test_graded_wage_paid_alike_at_both_tiers() {
    let mut w = city_on();
    w.levers.tax_rate = 0.0;
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager);
    assert!(m.len() >= 2);
    let (full, stat) = (m[0], m[1]);
    for (a, lod) in [(full, Lod::Full), (stat, Lod::Statistical)] {
        if let Some(b) = w.comp_mut::<Brain>(a) {
            b.lod = lod;
        }
        if let Some(j) = w.comp_mut::<Job>(a) {
            j.days_unpaid = 1;
        }
    }
    let gross = |w: &World, a| wages::gross_wage(w, w.comp::<Job>(a).expect("job"));
    let (gf, gs) = (gross(&w, full), gross(&w, stat));
    let paid_full = economy::collect_wage(&mut w, full);
    let paid_stat = economy::collect_wage(&mut w, stat);
    assert_eq!(paid_full, gf, "the Full tier pays the graded gross");
    assert_eq!(paid_stat, gs, "the Statistical tier pays the graded gross");
    let base = (w.config.wage(manager) as f32 * w.config.classpay.grade_mult[3]).round() as i64;
    assert!(gf >= base * 8 / 10, "a Manager's gross ({gf}) carries the grade ({base})");
}

/// The margin rule prices a graded place at its grade (the pool split):
/// `post_wage` is the role's wage times the base grade's multiple at a
/// corp's workplace, the plain wage elsewhere or with class pay off.
#[test]
fn test_post_wage_priced_at_grade() {
    let w = city_on();
    let manager = role(&w, "manager");
    let farm = w
        .buildings_of_kind(citysim::BuildingKind::Farm)
        .iter()
        .copied()
        .find(|&b| w.corp_of_building(b).is_some())
        .expect("a corp Farm");
    let m = w.config.classpay.grade_mult;
    assert_eq!(classpay::post_wage(&w, farm, manager), w.config.wage(manager) as f32 * m[3]);
    assert_eq!(classpay::post_wage(&w, farm, Role::Farmer), w.config.wage(Role::Farmer) as f32);
    let off = World::new(42, Config::load().dole_city());
    assert_eq!(classpay::post_wage(&off, farm, manager), off.config.wage(manager) as f32);
}

/// Every adult but `keep` leaves the labour pool (emigrating).
fn only_candidates(w: &mut World, keep: &[EntityId]) {
    for a in w.citizens() {
        if !keep.contains(&a) {
            if let Some(b) = w.comp_mut::<Brain>(a) {
                b.emigrating = true;
            }
        }
    }
}

fn set_education(w: &mut World, a: EntityId, origin: u8, education: f32) {
    w.insert(a, Schooling { origin, education });
}

/// Graded hiring is by merit: the Spire-schooled candidate over the Sump
/// one; a long-tenured worker at the same corp can win the post over a
/// better-schooled outsider (mobility), and leaves its old post re-posted.
#[test]
fn test_graded_vacancy_goes_to_merit_and_tenure_moves_up() {
    let mut w = city_on();
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager);
    let post = w.comp::<Job>(m[0]).and_then(|j| j.employer).expect("a Manager's workplace");
    let corp = w.corp_of_building(post).expect("a corp's");
    let jobless: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| {
            demography::is_adult(&w, a) && !w.has::<Job>(a) && w.comp::<Brain>(a).is_some_and(|b| !b.emigrating)
                // Lives at work phase 6: one the post's gate admits (not
                // retired, not too frail).
                && citysim::systems::careers::age_ok(&w, a, manager)
        })
        .take(2)
        .collect();
    assert_eq!(jobless.len(), 2);
    let (spire, sump) = (jobless[0], jobless[1]);
    set_education(&mut w, spire, 3, 0.95);
    set_education(&mut w, sump, 1, 0.15);
    only_candidates(&mut w, &[spire, sump]);
    let ms = classpay::merit(&w, spire, post, manager);
    let mu = classpay::merit(&w, sump, post, manager);
    assert!(ms > mu, "schooling leads: {ms} vs {mu}");
    assert_eq!(classpay::pick_graded(&w, post, manager), Some(spire));

    // Mobility: a labour-grade worker of the same corp, 200 days in, with
    // the post's skill at the top, against an outsider schooled at 0.55.
    w.tick = 400 * TICKS_PER_DAY;
    let insider = holders(&w, |j| j.grade == 0 && j.employer.and_then(|e| w.corp_of_building(e)) == Some(corp))[0];
    let hired = w.tick - 200 * TICKS_PER_DAY;
    if let Some(j) = w.comp_mut::<Job>(insider) {
        j.hired_tick = hired;
    }
    if let Some(b) = w.comp_mut::<Brain>(insider) {
        b.emigrating = false;
    }
    if let Some(s) = w.comp_mut::<citysim::Skills>(insider) {
        s.persuasion = 1.0;
        s.knowledge = 1.0;
    }
    set_education(&mut w, insider, 1, 0.3);
    set_education(&mut w, spire, 2, 0.55);
    if let Some(s) = w.comp_mut::<citysim::Skills>(spire) {
        s.persuasion = 0.0;
        s.knowledge = 0.0;
    }
    let mi = classpay::merit(&w, insider, post, manager);
    let mo = classpay::merit(&w, spire, post, manager);
    assert!(mi > mo, "tenure and skill carry the insider: {mi} vs {mo}");
    assert_eq!(classpay::pick_graded(&w, post, manager), Some(insider));
    // The move: the insider's old post is vacated (posted), the new one held.
    let old = w.comp::<Job>(insider).and_then(|j| j.employer.map(|e| (e, j.role))).expect("job");
    let open = |w: &World| w.vacancies.get(&old.0).map_or(0, |v| v.iter().filter(|&&r| r == old.1).count());
    let before = open(&w);
    assert!(classpay::leave_for_promotion(&mut w, insider));
    assert_eq!(open(&w), before + 1, "the old post is re-posted");
    demography::hire(&mut w, insider, post, manager);
    assert_eq!(w.comp::<Job>(insider).map(|j| j.grade), Some(3));
}

/// No promotion to skilled before `promote_days` of tenure; after it the
/// building's skilled slots fill, at the skilled wage.
#[test]
fn test_promotion_waits_for_tenure() {
    let mut w = city_on();
    w.tick = 100 * TICKS_PER_DAY;
    let farmers =
        holders(&w, |j| j.role == Role::Farmer && j.employer.is_some_and(|e| w.corp_of_building(e).is_some()));
    let farm = w.comp::<Job>(farmers[0]).and_then(|j| j.employer).expect("farm");
    let staff: Vec<EntityId> = holders(&w, |j| j.employer == Some(farm) && j.role == Role::Farmer);
    assert!(staff.len() >= 4, "a Farm with a crew");
    let now = w.tick;
    let wage0 = w.config.wage(Role::Farmer);
    for &a in &staff {
        if let Some(j) = w.comp_mut::<Job>(a) {
            j.grade = 0;
            j.hired_tick = now;
            j.wage_per_day = wage0;
        }
    }
    classpay::promote_skilled(&mut w, false);
    assert!(staff.iter().all(|&a| w.comp::<Job>(a).is_some_and(|j| j.grade == 0)), "no promotion before tenure");
    for &a in &staff {
        if let Some(j) = w.comp_mut::<Job>(a) {
            j.hired_tick = now - 40 * TICKS_PER_DAY;
        }
    }
    classpay::promote_skilled(&mut w, false);
    let skilled: Vec<EntityId> =
        staff.iter().copied().filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.grade == 1)).collect();
    let slots = (staff.len() as f32 * w.config.classpay.skilled_share).floor() as usize;
    assert_eq!(skilled.len(), slots, "the skilled slots fill after tenure");
    let wage = (wage0 as f32 * w.config.classpay.grade_mult[1]).round() as i64;
    assert!(skilled.iter().all(|&a| w.comp::<Job>(a).is_some_and(|j| j.wage_per_day == wage)));
}

/// Layoffs take a professional or management post last: the newest hire
/// the margin rule lays off is not a Manager while anyone else could go.
#[test]
fn test_layoff_spares_graded_posts() {
    let mut w = city_on();
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager)[0];
    let corp = w.comp::<Job>(m).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e)).expect("corp");
    if let Some(j) = w.comp_mut::<Job>(m) {
        j.hired_tick = 1_000_000;
    }
    let (who, _) = wages::newest_hire(&w, corp).expect("a hire to lay off");
    assert_ne!(who, m, "the Manager hired last is not the one laid off");
    assert!(w.comp::<Job>(who).is_some_and(|j| j.grade < classpay::PROFESSIONAL));
}

/// A corp's replacement exec comes from its management, leaves the post,
/// and logs a promotion.
#[test]
fn test_exec_from_management() {
    let mut w = city_on();
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager);
    let corp_of =
        |w: &World, a: EntityId| w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e));
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| classpay::exec_from_ranks(&w, c).is_some() && m.iter().any(|&a| corp_of(&w, a) == Some(c)))
        .expect("a corp with a Manager old enough to run it");
    let pick = classpay::exec_from_ranks(&w, corp).expect("pick");
    assert_eq!(w.comp::<Job>(pick).map(|j| j.grade), Some(3), "a Manager first");
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.exec = None;
    }
    w.run_ticks(TICKS_PER_DAY + 1);
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec).expect("a new exec");
    assert!(!w.has::<Job>(exec), "the exec left the post");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Promoted && e.actors.first() == Some(&exec)));
}

/// Backgrounds are pure hashes (the same seed, the same Schooling) and the
/// classes' mean education is ordered Spire > Mid > Sump > Dreg.
#[test]
fn test_schooling_deterministic_and_ordered() {
    let a = city_on();
    let b = city_on();
    let mut sums = [(0.0f32, 0u32); 4];
    for id in a.citizens() {
        let s = a.comp::<Schooling>(id).copied().expect("every citizen schooled");
        assert_eq!(b.comp::<Schooling>(id).copied(), Some(s));
        sums[usize::from(s.origin)].0 += s.education;
        sums[usize::from(s.origin)].1 += 1;
    }
    let mean = |i: usize| sums[i].0 / sums[i].1.max(1) as f32;
    assert!(mean(3) > mean(2) && mean(2) > mean(1) && mean(1) > mean(0), "{sums:?}");
}

/// A graded post holder keeps its Home's tier when it relocates; a labour
/// post holder moves anywhere.
#[test]
fn test_graded_keep_their_tier() {
    let w = city_on();
    let manager = role(&w, "manager");
    let housed = |a: &EntityId| w.comp::<Household>(*a).and_then(|h| h.home);
    let m = holders(&w, |j| j.role == manager).into_iter().find(|a| housed(a).is_some()).expect("a housed Manager");
    let home = housed(&m).expect("home");
    let tier = w.comp::<Building>(home).map(|b| b.tier).expect("tier");
    assert_eq!(classpay::min_home_tier(&w, m, home), tier);
    let l = holders(&w, |j| j.grade == 0).into_iter().find(|a| housed(a).is_some()).expect("a housed labourer");
    assert_eq!(classpay::min_home_tier(&w, l, housed(&l).expect("home")), 0);
}

// ---------------------------------------------------------------------------
// P3 (Dylan 2026-10-10): the premium from capital and profit
// ---------------------------------------------------------------------------

/// A graded wage pays in full out of the corp's purse (no coin made or
/// lost), but only its labour-rate part counts in the labour pool's payroll
/// window: the premium above it is noted apart (`Corp.prem_today`).
#[test]
fn test_premium_noted_apart_and_coins_conserved() {
    let mut w = city_on();
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager)[0];
    let corp = w.comp::<Job>(m).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e)).expect("corp");
    if let Some(j) = w.comp_mut::<Job>(m) {
        j.days_unpaid = 1;
    }
    let before = citysim::systems::econ::identity(&w);
    let (pay0, prem0) = w.comp::<Corp>(corp).map(|c| (c.pay_today, c.prem_today)).expect("corp");
    let gross = wages::gross_wage(&w, w.comp::<Job>(m).expect("job"));
    economy::collect_wage(&mut w, m);
    let (pay1, prem1) = w.comp::<Corp>(corp).map(|c| (c.pay_today, c.prem_today)).expect("corp");
    let premium = prem1 - prem0;
    let mult = w.config.classpay.grade_mult[3];
    assert_eq!(premium, gross - (gross as f32 / mult).round() as i64, "the premium is the part above the labour rate");
    assert_eq!(pay1 - pay0 + premium, gross, "labour part + premium = the gross paid");
    assert!(premium > 0 && pay1 - pay0 > 0);
    assert_eq!(citysim::systems::econ::identity(&w), before, "no coin made or lost");
}

/// A corp whose profit and capital payout cannot cover its premium posts and
/// hires no graded place and withdraws the open ones; labour posts go on.
#[test]
fn test_unfunded_premium_stops_graded_hiring() {
    use std::collections::VecDeque;
    let mut w = city_on();
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager)[0];
    let post = w.comp::<Job>(m).and_then(|j| j.employer).expect("post");
    let corp = w.corp_of_building(post).expect("corp");
    let pin = |w: &mut World, prem: i64, cash: i64| {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.rev = VecDeque::from(vec![1_000; wages::WINDOW_DAYS]);
        c.pay = VecDeque::from(vec![500; wages::WINDOW_DAYS]);
        c.prem = VecDeque::from(vec![prem; wages::WINDOW_DAYS]);
        c.cashflow = VecDeque::from(vec![cash; 14]);
        c.treasury = 0;
    };
    // Profitable: 300 a day after a 100 premium.
    pin(&mut w, 100, 300);
    assert!(classpay::premium_room(&w, corp).is_some_and(|r| r > 0.0));
    assert!(classpay::premium_funded(&w, post, manager));
    // Losing money with no capital: the premium is unfunded.
    pin(&mut w, 100, -200);
    assert!(classpay::premium_room(&w, corp).is_some_and(|r| r < 0.0));
    assert!(!classpay::premium_funded(&w, post, manager));
    assert!(classpay::premium_funded(&w, post, Role::Farmer), "labour hiring is not gated");
    w.vacancies.entry(post).or_default().push(manager);
    w.vacancies.entry(post).or_default().push(Role::Clerk);
    assert!(classpay::withdraw_unfunded(&mut w, corp) >= 1);
    let open = w.vacancies.get(&post).cloned().unwrap_or_default();
    assert!(!open.contains(&manager), "the graded vacancy is withdrawn");
    assert!(open.contains(&Role::Clerk), "the labour vacancy stands");
    // An unfunded premium draws on at most `premium_payout_share` of the payout.
    let share = w.config.classpay.premium_payout_share;
    assert_eq!(classpay::labour_payout(&w, corp, 50.0), 50.0 * (1.0 - share), "labour keeps its half of the payout");
}

/// The thin ladder: a Sump-born skilled worker's night school is slow
/// (a month moves its education under 0.01, and it is still out-ranked by a
/// Spire-born outsider for its corp's Manager post), but three sim years in
/// the post carry it past that outsider. Deterministic: the same days, the
/// same education.
#[test]
fn test_night_school_ladder_exists_and_is_slow() {
    let mut w = city_on();
    let manager = role(&w, "manager");
    let post = holders(&w, |j| j.role == manager)
        .into_iter()
        .find_map(|m| w.comp::<Job>(m).and_then(|j| j.employer))
        .expect("a Manager's workplace");
    let corp = w.corp_of_building(post).expect("corp");
    let worker = holders(&w, |j| j.grade <= 1 && j.employer.and_then(|e| w.corp_of_building(e)) == Some(corp))[0];
    let outsider = w
        .citizens()
        .into_iter()
        .find(|&a| demography::is_adult(&w, a) && !w.has::<Job>(a) && a != worker)
        .expect("a jobless adult");
    set_education(&mut w, worker, 1, 0.25);
    set_education(&mut w, outsider, 3, 0.8);
    for (a, s) in [(worker, 0.5), (outsider, 0.3)] {
        if let Some(k) = w.comp_mut::<citysim::Skills>(a) {
            k.persuasion = s;
            k.knowledge = s;
        }
    }
    let start = 10 * TICKS_PER_DAY;
    w.tick = start;
    if let Some(j) = w.comp_mut::<Job>(worker) {
        j.grade = 1;
        j.hired_tick = start;
    }
    // A month of night school.
    for d in 1..=30 {
        w.tick = start + d * TICKS_PER_DAY;
        classpay::night_school(&mut w);
    }
    let edu = w.comp::<Schooling>(worker).map(|s| s.education).expect("schooling");
    assert!(edu > 0.25 && edu < 0.26, "a month moves education a little: {edu}");
    assert!(
        classpay::merit(&w, worker, post, manager) < classpay::merit(&w, outsider, post, manager),
        "after a month the Spire-born outsider still ranks first"
    );
    // Three sim years.
    for d in 31..=3 * 365 {
        w.tick = start + d * TICKS_PER_DAY;
        classpay::night_school(&mut w);
    }
    let edu3 = w.comp::<Schooling>(worker).map(|s| s.education).expect("schooling");
    assert!(edu3 > 0.5 && edu3 <= w.config.classpay.edu_cap, "three years of night school: {edu3}");
    assert!(
        classpay::merit(&w, worker, post, manager) > classpay::merit(&w, outsider, post, manager),
        "after three years the Sump-born worker ranks first"
    );
    // Labour posts learn nothing; the cap holds.
    let labourer = holders(&w, |j| j.grade == 0)[0];
    set_education(&mut w, labourer, 1, 0.25);
    classpay::night_school(&mut w);
    assert_eq!(w.comp::<Schooling>(labourer).map(|s| s.education), Some(0.25));
    for _ in 0..5000 {
        classpay::night_school(&mut w);
    }
    assert_eq!(w.comp::<Schooling>(worker).map(|s| s.education), Some(w.config.classpay.edu_cap));
}

/// The status sink: an adult over `status_floor` spends `status_frac` of the
/// excess at midnight, at the Spire and in the corps' shops; every coin
/// lands in an owner's purse (the coin identity holds) and the night out
/// gives fun. The poor spend nothing; class pay off, nobody does.
#[test]
fn test_status_sink_spends_the_excess_into_owners() {
    let mut w = city_on();
    let rich = holders(&w, |j| j.grade >= 2)[0];
    let poor = holders(&w, |j| j.grade == 0)[0];
    let floor = w.config.classpay.status_floor;
    for (a, c) in [(rich, 1_000), (poor, floor)] {
        if let Some(x) = w.comp_mut::<Wallet>(a) {
            x.coins = c;
        }
    }
    if let Some(n) = w.comp_mut::<citysim::Needs>(rich) {
        n.fun = 0.0;
    }
    let before = citysim::systems::econ::identity(&w);
    let spent = classpay::status_daily(&mut w);
    let budget = (w.config.classpay.status_frac * (1_000 - floor) as f32).floor() as i64;
    let left = w.comp::<Wallet>(rich).map(|x| x.coins).expect("wallet");
    assert!(spent >= 1_000 - left && 1_000 - left > 0 && 1_000 - left <= budget, "spent {} of {budget}", 1_000 - left);
    assert_eq!(w.comp::<Wallet>(poor).map(|x| x.coins), Some(floor), "the poor spend nothing");
    assert_eq!(citysim::systems::econ::identity(&w), before, "every coin reaches an owner");
    let off = &mut World::new(42, Config::load().dole_city());
    assert_eq!(classpay::status_daily(off), 0, "no status sink without wages");
}

/// The Treasury's census (diagnostic, `#[ignore]`): seed 42, `CLASSPAY_DAYS`
/// days on `CITYSIM_ASSETS`, the Treasury's inflows and outflows by flow per
/// window (from `World::probe`), and the residual no flow explains.
#[test]
#[ignore = "tier: diag"]
fn treasury_census() {
    let days: u64 = std::env::var("CLASSPAY_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(365);
    let mut w = World::new(42, Config::load().dole_city());
    let t0 = w.treasury().map_or(0, |t| t.coins);
    let mut last = (t0, w.probe.treasury_in.clone(), w.probe.treasury_out.clone());
    let windows = [0u64, 30, 120, 240, days];
    for pair in windows.windows(2) {
        w.run_ticks((pair[1] - pair[0]) * TICKS_PER_DAY);
        let t = w.treasury().map_or(0, |t| t.coins);
        let n = (pair[1] - pair[0]) as f64;
        println!("== days {}-{}: Treasury {} -> {} ({:+.1}/day)", pair[0], pair[1], last.0, t, (t - last.0) as f64 / n);
        let mut net = 0i64;
        for (dir, now, then) in [("in", &w.probe.treasury_in, &last.1), ("out", &w.probe.treasury_out, &last.2)] {
            let mut rows: Vec<(i64, &String)> =
                now.iter().map(|(k, &v)| (v - then.get(k).copied().unwrap_or(0), k)).filter(|(d, _)| *d != 0).collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.0));
            for (d, k) in rows {
                println!("  {dir:>3} {k:<36} {:>9.1}/day", d as f64 / n);
                if !k.contains(": of it") {
                    net += if dir == "in" { d } else { -d };
                }
            }
        }
        println!("  residual (no flow) {:+.1}/day", ((t - last.0) - net) as f64 / n);
        last = (t, w.probe.treasury_in.clone(), w.probe.treasury_out.clone());
    }
}

/// The wage rule's census (diagnostic, `#[ignore]`): seed 42, `CLASSPAY_DAYS`
/// days on `CITYSIM_ASSETS`. Per window: the corps' summed 7-day revenue R,
/// labour payroll P and target P*, the `WageMoved` steps up by reason (a
/// vacancy stood unfilled, or payroll under target), the steps down, the
/// payroll-weighted `wage_rev`, the open vacancies and the jobless adults.
#[test]
#[ignore = "tier: diag"]
fn wage_rule_census() {
    let days: u64 = std::env::var("CLASSPAY_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(365);
    let mut w = World::new(42, Config::load().dole_city());
    let windows = [0u64, 30, 120, 240, days];
    let mut cursor = 0u64;
    for pair in windows.windows(2) {
        let (mut r, mut p, mut t, mut vac, mut jobless) = (0.0f64, 0.0f64, 0.0f64, 0u64, 0u64);
        let (mut up_short, mut up_pay, mut down) = (0u32, 0u32, 0u32);
        for _ in pair[0]..pair[1] {
            w.run_ticks(TICKS_PER_DAY);
            for e in w.events.iter().filter(|e| e.id >= cursor && e.kind == citysim::EventKind::WageMoved) {
                let up = e
                    .text
                    .split(" (was x")
                    .next()
                    .and_then(|s| s.rsplit('x').next())
                    .and_then(|s| s.parse::<f32>().ok());
                let was =
                    e.text.split("(was x").nth(1).and_then(|s| s.split(';').next()).and_then(|s| s.parse::<f32>().ok());
                match (up, was) {
                    (Some(n), Some(o)) if n > o && e.text.contains("unfilled") => up_short += 1,
                    (Some(n), Some(o)) if n > o => up_pay += 1,
                    _ => down += 1,
                }
            }
            cursor = w.events.back().map_or(cursor, |e| e.id + 1);
            for c in w.corps() {
                if let Some((rr, pp)) = wages::windows(&w, c) {
                    r += f64::from(rr);
                    p += f64::from(pp);
                    t += f64::from(wages::target(&w, c).unwrap_or(0.0));
                }
            }
            vac += u64::from(wages::vacancies_open(&w));
            jobless +=
                w.citizens().into_iter().filter(|&a| demography::is_adult(&w, a) && !w.has::<Job>(a)).count() as u64;
        }
        let n = (pair[1] - pair[0]) as f64;
        println!(
            "days {:>3}-{:<3}: R {:>7.0} P {:>7.0} P* {:>7.0} /day | ups: unfilled {up_short}, payroll {up_pay}; downs {down} | wage_rev {:.2} | open vacancies {:.1} | jobless adults {:.0}",
            pair[0],
            pair[1],
            r / n,
            p / n,
            t / n,
            wages::mean_wage_rev(&w),
            vac as f64 / n,
            jobless as f64 / n
        );
    }
}

/// P3 (Dylan): the graded premium draws on profit plus at most
/// `premium_payout_share` of the capital payout; the rest always reaches
/// labour's P*. A premium the profit covers takes nothing from the payout; a
/// premium past profit plus that share is unfunded (graded hiring pauses).
#[test]
fn test_premium_takes_at_most_its_share_of_the_payout() {
    use std::collections::VecDeque;
    let mut w = city_on();
    let share = w.config.classpay.premium_payout_share;
    assert!(share > 0.0 && share < 1.0, "the shipped share splits the payout");
    let manager = role(&w, "manager");
    let m = holders(&w, |j| j.role == manager)[0];
    let post = w.comp::<Job>(m).and_then(|j| j.employer).expect("post");
    let corp = w.corp_of_building(post).expect("corp");
    let pin = |w: &mut World, prem: i64, cash: i64| {
        let c = w.comp_mut::<Corp>(corp).expect("corp");
        c.prem = VecDeque::from(vec![prem; wages::WINDOW_DAYS]);
        c.cashflow = VecDeque::from(vec![cash; 14]);
    };
    let payout = 400.0f32;
    // Profit covers the premium: labour keeps the whole payout.
    pin(&mut w, 100, 50);
    assert_eq!(classpay::labour_payout(&w, corp, payout), payout);
    // No profit, a premium of 100: drawn from the payout, labour keeps the rest.
    pin(&mut w, 100, -100);
    assert_eq!(classpay::labour_payout(&w, corp, payout), payout - 100.0);
    // No profit, a premium past the share: the premium takes only its share.
    pin(&mut w, 1_000, -1_000);
    assert_eq!(classpay::labour_payout(&w, corp, payout), payout * (1.0 - share), "labour keeps the rest");
    // And the premium's room counts only that share: here it is unfunded.
    let c = w.comp_mut::<Corp>(corp).expect("corp");
    c.rev = VecDeque::from(vec![1_000; wages::WINDOW_DAYS]);
    c.pay = VecDeque::from(vec![500; wages::WINDOW_DAYS]);
    let (r, p) = wages::windows(&w, corp).expect("windows");
    let full = wages::capital_payout(&w, corp, r, p);
    let room = classpay::premium_room(&w, corp).expect("room");
    assert!((room - (share * full - 1_000.0)).abs() < 1e-2, "room {room} = share x payout {full} - premium");
}
