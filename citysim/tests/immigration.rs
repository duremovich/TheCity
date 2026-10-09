//! Jobs and room P6 (docs/JOBS_V2.md § 4, plan J19-J21): immigration that
//! answers the city. The Harris-Todaro week (a pull of 0 brings no one),
//! offers for posts open `offer_days` (capped, oldest first; the migrant
//! holds the job at spawn and lives nearest it), who comes (18-45, the
//! seeding skill draw, a spouse, children behind a key) and the coin
//! identity (every arrival's coins cross in from the World). Each migrant is
//! an abstract event: an entity spawned at the map edge with integer purses.

use std::collections::VecDeque;

use citysim::econ::MigrantDay;
use citysim::systems::{demography, econ};
use citysim::time::DAYS_PER_YEAR;
use citysim::{
    Building, BuildingKind, Child, Config, EntityId, EventKind, Household, Identity, Job, PlayerCommand, Role, Sex,
    Skills, Wallet, World, TICKS_PER_DAY,
};

/// The wages + no-net city (the switches the phase is read under).
fn config() -> Config {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    cfg
}

fn city(cfg: Config) -> World {
    let w = World::new(42, cfg);
    assert!(citysim::systems::wages::on(&w), "wages on for the tests");
    w
}

fn home(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Household>(id).and_then(|h| h.home)
}

fn surname(w: &World, id: EntityId) -> String {
    w.comp::<Identity>(id).map(|i| i.name.rsplit(' ').next().unwrap_or("").to_string()).unwrap_or_default()
}

/// Standing Farms, ascending.
fn farms(w: &World) -> Vec<EntityId> {
    w.buildings_of_kind(BuildingKind::Farm)
        .iter()
        .copied()
        .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect()
}

/// Clear the city's open posts and post `n` Farm vacancies stamped `ages`
/// days ago (one per Farm, in Farm order).
fn post_aged(w: &mut World, ages: &[u64]) -> Vec<EntityId> {
    w.vacancies.clear();
    w.econ.vacancy_since.clear();
    let fs = farms(w);
    assert!(fs.len() >= ages.len(), "enough Farms ({})", fs.len());
    let now = w.tick;
    for (i, &age) in ages.iter().enumerate() {
        w.vacancies.entry(fs[i]).or_default().push(Role::Farmer);
        w.econ.vacancy_since.insert((fs[i], Role::Farmer), now - age * TICKS_PER_DAY);
    }
    fs[..ages.len()].to_vec()
}

/// J19: `E = gross_mean × employed ÷ free adults` over the 7-day means,
/// `pull = clamp((E − R) ÷ R, 0, pull_cap)`; the lever pins `R`.
#[test]
fn test_expected_wage_and_pull() {
    let mut w = city(config());
    w.econ.migrant_days = VecDeque::from(vec![
        MigrantDay { gross_mean: 6.0, employed: 500, free_adults: 1000 },
        MigrantDay { gross_mean: 6.0, employed: 700, free_adults: 1000 },
    ]);
    // E = 6 × 600 ÷ 1000 = 3.6.
    assert!((demography::expected_wage(&w) - 3.6).abs() < 1e-4);
    // The default R 3.5: pull (3.6 − 3.5) ÷ 3.5.
    assert!((demography::pull(&w) - 0.1 / 3.5).abs() < 1e-4);
    w.push_command(PlayerCommand::SetOutsideWage(3.0));
    w.apply_commands();
    assert_eq!(demography::outside_wage(&w), 3.0);
    assert!((demography::pull(&w) - 0.2).abs() < 1e-4);
    // Over the cap: clamped at pull_cap (1.0).
    w.econ.outside_wage_pin = Some(1.0);
    assert_eq!(demography::pull(&w), 1.0);
    let week = demography::pull_week(&w);
    let beds = (demography::empty_beds(&w) as f32 / 60.0).clamp(0.25, 1.0);
    let factor = citysim::systems::classes::immigration_factor(&w);
    assert_eq!(week, (20.0 * beds * factor).round() as u32);
}

/// J19: a pull of 0 (the city's expected wage under the outside's) brings
/// no Harris-Todaro migrant; a high pull brings some.
#[test]
fn test_pull_zero_posts_no_week() {
    let mut w = city(config());
    w.push_command(PlayerCommand::SetOutsideWage(1000.0));
    w.run_ticks(7 * TICKS_PER_DAY + 1);
    assert_eq!(w.econ.migrant_days.len(), 7, "a week of the labour market noted");
    assert_eq!(demography::pull(&w), 0.0);
    assert_eq!(demography::pull_week(&w), 0);
    let pulled: u32 =
        w.stats.history.iter().map(|r| r.jobs.migrants_pull).sum::<u32>() + w.stats.current.jobs.migrants_pull;
    assert_eq!(pulled, 0, "no migrant drawn at pull 0");
    // The outside collapses: the city's wage now beats it.
    w.econ.outside_wage_pin = Some(0.1);
    assert!(demography::pull(&w) > 0.0);
    let week = demography::pull_week(&w);
    assert!(week > 0, "a full pull draws a week");
    w.run_ticks(7 * TICKS_PER_DAY);
    let pulled: u32 =
        w.stats.history.iter().map(|r| r.jobs.migrants_pull).sum::<u32>() + w.stats.current.jobs.migrants_pull;
    assert!(pulled > 0, "a week arrived (the pull's week read {week} a week before)");
}

/// J20: offers go to places open `offer_days` or more, oldest first, at
/// most `offers_max` a week.
#[test]
fn test_offers_capped_and_oldest_first() {
    let mut cfg = config();
    cfg.demography.offers_max = 3;
    let mut w = city(cfg);
    w.run_ticks(10 * TICKS_PER_DAY);
    // Ages in days: 0, 1, 2 are too young; 3..=7 are due.
    let fs = post_aged(&mut w, &[0, 5, 1, 7, 3, 2, 6, 4]);
    let due = demography::offers_due(&w);
    assert_eq!(due, vec![(fs[3], Role::Farmer), (fs[6], Role::Farmer), (fs[1], Role::Farmer)]);
    w.config.demography.offers_max = 10;
    let all: Vec<EntityId> = demography::offers_due(&w).into_iter().map(|(b, _)| b).collect();
    assert_eq!(all, vec![fs[3], fs[6], fs[1], fs[7], fs[4]], "every due post, oldest first, none under 3 days");
}

/// J20: the offer's migrant holds the job at spawn (the vacancy closes),
/// lives in the Block with room nearest the workplace, and its role skill
/// is at least `offer_skill`.
#[test]
fn test_offer_migrant_holds_the_job_and_lives_nearest() {
    let mut cfg = config();
    cfg.demography.p_spouse = 0.0;
    let mut w = city(cfg);
    w.run_ticks(10 * TICKS_PER_DAY);
    let fs = post_aged(&mut w, &[5]);
    let farm = fs[0];
    let door = w.comp::<Building>(farm).expect("farm").door;
    // The nearest Block with a free bed, by brute force.
    let want = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter_map(|h| {
            let b = w.comp::<Building>(h)?;
            (!b.demolished && !b.derelict && w.residents_of(h).len() < usize::from(b.capacity))
                .then(|| (b.door.manhattan(door), w.residents_of(h).len(), h))
        })
        .min()
        .map(|(_, _, h)| h);
    assert!(want.is_some(), "a Block with room");
    let before = w.stats.current.jobs.migrants_offer;
    let a = demography::spawn_immigrant_for(&mut w, farm, Role::Farmer).expect("an offer migrant");
    let job = w.comp::<Job>(a).expect("hired at spawn");
    assert_eq!((job.employer, job.role), (Some(farm), Role::Farmer));
    assert!(!w.vacancies.contains_key(&farm), "the vacancy closed");
    assert!(!w.econ.vacancy_since.contains_key(&(farm, Role::Farmer)));
    assert_eq!(home(&w, a), want, "housed nearest the workplace");
    assert!(w.comp::<Skills>(a).expect("skills").farming >= 0.4, "the role skill floored");
    assert_eq!(w.stats.current.jobs.migrants_offer, before + 1);
    let text = format!("{} arrived for a Vat Tech job at", w.name_of(a));
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Immigration && e.text.starts_with(&text)),
        "the offer's event"
    );
    // The place is filled: a second offer for it finds nothing.
    assert!(demography::spawn_immigrant_for(&mut w, farm, Role::Farmer).is_none());
}

/// J21: `p_spouse` brings a spouse of the other sex into the same Block,
/// with the same surname and a Spouse edge; no children by default.
#[test]
fn test_spouse_arrives_with_the_migrant() {
    let mut cfg = config();
    cfg.demography.p_spouse = 1.0;
    let mut w = city(cfg);
    w.run_ticks(TICKS_PER_DAY);
    let imm = w.stats.current.immigrants;
    let (a, n) = demography::spawn_migrant(&mut w, None);
    assert_eq!(n, 2);
    let s = w.spouse_of(a).expect("a spouse");
    assert_eq!(home(&w, a), home(&w, s));
    assert!(home(&w, a).is_some(), "housed");
    let sex = |id| w.comp::<Identity>(id).map(|i| i.sex);
    assert_ne!(sex(a), sex(s));
    assert!(matches!(sex(a), Some(Sex::Male | Sex::Female)));
    assert_eq!(surname(&w, a), surname(&w, s));
    assert_eq!(w.stats.current.immigrants, imm + 2);
    assert!(citysim::systems::demography::children_of_agent(&w, a).is_empty(), "no children by default");
}

/// Dylan's open question, behind `[demography] migrant_children` (0 by
/// default): a couple may bring children, in their Block, as Children.
#[test]
fn test_children_behind_the_key() {
    let mut cfg = config();
    cfg.demography.p_spouse = 1.0;
    cfg.demography.migrant_children = 3;
    let mut w = city(cfg);
    w.run_ticks(TICKS_PER_DAY);
    // Room for the families: the emptiest Block takes forty.
    let roomy = demography::emptiest_home_near(&w, None, 1).expect("a Block with room");
    w.comp_mut::<Building>(roomy).expect("block").capacity = 40;
    let mut kids = Vec::new();
    for _ in 0..8 {
        let (a, n) = demography::spawn_migrant(&mut w, None);
        let ks = demography::children_of_agent(&w, a);
        assert_eq!(n as usize, 2 + ks.len());
        for &k in &ks {
            assert!(w.has::<Child>(k));
            assert_eq!(home(&w, k), home(&w, a));
            assert!(w.comp::<Identity>(k).is_some_and(|i| i.age_days < demography::ADULT_AGE_DAYS));
            assert_eq!(w.comp::<Wallet>(k).map(|x| x.coins), Some(0));
        }
        kids.extend(ks);
    }
    assert!(!kids.is_empty(), "eight couples brought a child");
}

/// J21: migrants are 18-45 with the seeded residents' skill draw (not a
/// flat 0.1).
#[test]
fn test_who_comes() {
    let mut cfg = config();
    cfg.demography.p_spouse = 0.0;
    let mut w = city(cfg);
    w.run_ticks(TICKS_PER_DAY);
    let (lo, hi) = (w.config.world.skill_min, w.config.world.skill_max);
    let mut farming = Vec::new();
    for _ in 0..20 {
        let a = demography::spawn_immigrant(&mut w);
        let age = w.comp::<Identity>(a).expect("identity").age_days;
        assert!((demography::ADULT_AGE_DAYS..45 * DAYS_PER_YEAR as u32).contains(&age), "age {age}");
        let s = w.comp::<Skills>(a).expect("skills");
        for v in [s.stealth, s.fighting, s.farming] {
            assert!((lo..hi).contains(&v), "skill {v} in the seeding range");
        }
        farming.push(s.farming);
    }
    farming.dedup();
    assert!(farming.len() > 10, "drawn, not flat");
}

/// The coin identity holds over arrivals: every migrant's coins cross in
/// from the World (`Flow::Migrant`).
#[test]
fn test_migrants_keep_the_coin_identity() {
    let mut cfg = config();
    cfg.demography.p_spouse = 1.0;
    cfg.demography.migrant_children = 2;
    let mut w = city(cfg);
    w.run_ticks(10 * TICKS_PER_DAY);
    post_aged(&mut w, &[4, 6]);
    let before = econ::identity(&w);
    let mut ids = Vec::new();
    for (b, role) in demography::offers_due(&w) {
        ids.push(demography::spawn_immigrant_for(&mut w, b, role).expect("offer"));
    }
    for _ in 0..5 {
        ids.push(demography::spawn_migrant(&mut w, None).0);
    }
    ids.push(demography::spawn_immigrant(&mut w));
    assert_eq!(econ::identity(&w), before, "arrivals moved no coin out of thin air");
    assert!(ids.iter().all(|&a| w.comp::<Wallet>(a).is_some_and(|x| x.coins >= 15)), "every adult crossed in");
    // And over a week of the city with both paths running.
    w.econ.outside_wage_pin = Some(0.5);
    let start = econ::identity(&w);
    w.run_ticks(8 * TICKS_PER_DAY);
    assert_eq!(econ::identity(&w), start);
}
