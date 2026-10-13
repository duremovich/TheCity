//! Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier. The shipped city has
//! none; a flat gives the same person whatever was materialised before it; a
//! materialised household is housed, Statistical and paid for by the World
//! (the coin identity holds); the labour pool is bounded; a save resumes byte
//! for byte; and the parity-style check: a materialised resident looks like
//! one who was simulated all along (printed, loosely asserted).

use citysim::systems::{ambient, classes, econ};
use citysim::{
    save, Body, Brain, Building, BuildingKind, Config, EntityId, Household, Identity, Job, Lod, Mood, Needs,
    Personality, Skills, Wallet, World, TICKS_PER_DAY,
};

/// The shipped city with `per_slot` Ambient flats per household slot.
fn ambient_world(seed: u64, per_slot: f32) -> World {
    let mut c = Config::load();
    c.ambient.flats_per_slot = per_slot;
    World::new(seed, c)
}

/// The person a flat gives (not its state): name, sex, age, personality,
/// skills, body.
fn person(w: &World, id: EntityId) -> String {
    let i = w.comp::<Identity>(id).expect("identity");
    let b = w.comp::<Body>(id).expect("body");
    format!(
        "{} {:?} {} | {:?} | {:?} | {} {}",
        i.name,
        i.sex,
        i.age_days,
        w.comp::<Personality>(id).expect("personality"),
        w.comp::<Skills>(id).expect("skills"),
        b.strength,
        b.reflex
    )
}

#[test]
fn test_shipped_city_has_no_ambient_tier() {
    let w = World::new(42, Config::load());
    assert!(w.ambient.is_empty(), "the shipped map has no Ambient flats");
    assert!(!ambient::on(&w));
    let header = citysim::systems::stats::csv_header(&w);
    assert!(!header.contains("ambient_"), "no Ambient columns on the 2k city");
    let ron = save::to_ron(&w);
    assert!(!ron.contains("ambient"), "nothing Ambient in a 2k save");
}

#[test]
fn test_ambient_flats_from_the_ratio() {
    let w = ambient_world(42, 4.0);
    // Every map Home still a Home (the seeding passes turn a derelict Block
    // into the Unplugged's Chapel) has its flats.
    let map_homes = w.map.buildings.iter().filter(|d| d.kind == BuildingKind::Home).count();
    let homes = w.ambient.homes.len();
    assert!(homes <= map_homes && homes + 2 >= map_homes, "{homes} of {map_homes} map Homes");
    assert!(w
        .ambient
        .homes
        .iter()
        .all(|h| w.comp::<Building>(h.building).is_some_and(|b| b.kind == BuildingKind::Home)));
    assert_eq!(w.ambient.flats(), 4 * homes as u64);
    let occ = w.ambient.households() as f64 / w.ambient.flats() as f64;
    let per = w.ambient.adults() as f64 / w.ambient.households() as f64;
    println!("flats {} occupied {:.3} adults/household {:.3}", w.ambient.flats(), occ, per);
    assert!((occ - f64::from(w.config.ambient.occupancy)).abs() < 0.03, "occupancy {occ}");
    assert!((per - 1.0 - f64::from(w.config.ambient.couple_p)).abs() < 0.05, "adults per household {per}");
    let header = citysim::systems::stats::csv_header(&w);
    // M17 phase 1 appended the Ambient book's columns to the group; phase 2
    // puts the parents' group after it.
    let outside = citysim::stats::OutsideCols::header(w.outside.parents().count());
    let tail = format!(
        ",ambient_labour,ambient_food,ambient_food_wanted,ambient_food_bought,ambient_visits_wanted,ambient_visits_made,flow_ambient_food,flow_ambient_visits,ambient_appetite,{outside},ticks_per_sec"
    );
    assert!(header.ends_with(&tail), "{}", &header[header.len() - 200..]);
    let row = w.stats.current.csv_row();
    assert_eq!(
        row.split(',').count() + 18 + outside.split(',').count(),
        header.split(',').count(),
        "today's row gets its groups at the close"
    );
}

/// The flats' people are a function of the flat: two worlds materialising
/// the same flats in opposite orders (and one a day later) get the same people.
#[test]
fn test_materialise_same_person_any_order() {
    let homes = [0usize, 37, 211];
    let run = |order: &[usize], days: u64| -> Vec<(usize, Vec<String>)> {
        let mut w = ambient_world(7, 2.0);
        w.run_ticks(days * TICKS_PER_DAY);
        let mut out: Vec<(usize, Vec<String>)> = order
            .iter()
            .map(|&hi| {
                let adults = ambient::materialise(&mut w, hi);
                assert!(!adults.is_empty(), "home {hi} had a household");
                (hi, adults.iter().map(|&a| person(&w, a)).collect())
            })
            .collect();
        out.sort();
        out
    };
    let a = run(&homes, 0);
    let b = run(&[211, 0, 37], 0);
    let c = run(&[37, 211, 0], 1);
    for x in &a {
        println!("{x:?}");
    }
    assert_eq!(a, b, "the order of materialisation moved a person");
    assert_eq!(a, c, "a day later moved a person");
    // The same seed twice: the same world, byte for byte.
    let mut x = ambient_world(7, 2.0);
    let mut y = ambient_world(7, 2.0);
    for hi in homes {
        ambient::materialise(&mut x, hi);
        ambient::materialise(&mut y, hi);
    }
    assert_eq!(save::to_ron(&x), save::to_ron(&y));
}

/// A materialised household lives in its flat's Home, Statistical, with a bed
/// added; its savings cross in from the World (the identity holds); the
/// Home's counts move.
#[test]
fn test_materialise_houses_and_keeps_the_books() {
    let mut w = ambient_world(42, 4.0);
    w.run_ticks(TICKS_PER_DAY + 7);
    let before = econ::identity(&w);
    let pop = w.population();
    let hi = 5;
    let home = w.ambient.homes[hi].clone();
    let cap = w.comp::<Building>(home.building).expect("home").capacity;
    let adults = ambient::materialise(&mut w, hi);
    assert!(!adults.is_empty());
    assert_eq!(w.population(), pop + adults.len());
    assert_eq!(econ::identity(&w), before, "coins made or lost");
    let after = &w.ambient.homes[hi];
    assert_eq!(after.occupied, home.occupied - 1);
    assert_eq!(after.adults, home.adults - adults.len() as u32);
    assert!(after.cursor >= 1);
    let bd = w.comp::<Building>(home.building).expect("home");
    assert_eq!(bd.capacity, cap + adults.len() as u16, "a bed per adult");
    let coins: i64 = adults.iter().map(|&a| w.comp::<Wallet>(a).map_or(0, |x| x.coins)).sum();
    assert_eq!(w.ambient.today.coins_in, coins, "the wallets are the World's coins");
    for &a in &adults {
        assert_eq!(w.comp::<Household>(a).and_then(|h| h.home), Some(home.building));
        assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical));
        assert!(citysim::systems::demography::is_adult(&w, a));
    }
    if let [x, y] = adults[..] {
        assert_eq!(w.spouses.get(&x), Some(&y), "a couple is married");
    }
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Materialised));
    // It lives on as a resident.
    w.run_ticks(2 * TICKS_PER_DAY);
    assert!(adults.iter().all(|&a| w.is_alive(a)));
    assert_eq!(econ::identity(&w), before);
}

/// The labour pool fills a place no simulated adult took, up to the day's cap.
#[test]
fn test_labour_pool_is_bounded() {
    let mut w = ambient_world(42, 4.0);
    w.run_ticks(TICKS_PER_DAY + 7);
    let cap = w.config.per_capita_count(w.config.ambient.hires_per_day as usize);
    let employer = w.buildings_of_kind(BuildingKind::Farm)[0];
    let door = w.comp::<Building>(employer).expect("farm").door;
    let first = ambient::nearest_home(&w, door).map(|hi| w.ambient.homes[hi].building);
    let mut hired = Vec::new();
    for _ in 0..cap + 5 {
        if let Some(id) = ambient::hire_from_pool(&mut w, employer, citysim::Role::Farmer) {
            hired.push(id);
        }
    }
    assert_eq!(hired.len(), cap, "the day's cap");
    assert!(hired.iter().all(|&id| w.comp::<Job>(id).and_then(|j| j.employer) == Some(employer)));
    // The nearest Ambient Home first, and no Ambient Home with a household
    // left is nearer than it.
    let home0 = w.comp::<Household>(hired[0]).and_then(|h| h.home);
    assert_eq!(home0, first);
    let d0 = home0.and_then(|h| w.comp::<Building>(h)).expect("home").door.manhattan(door);
    let standing = |b: EntityId| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict);
    let nearer = w
        .ambient
        .homes
        .iter()
        .filter(|h| h.occupied > 0 && standing(h.building))
        .filter_map(|h| w.comp::<Building>(h.building))
        .any(|b| b.door.manhattan(door) < d0);
    assert!(!nearer || hired.len() > 1);
}

/// A city short of residents fills its places from the pool in the
/// midnight job search, at most the cap a day.
#[test]
fn test_job_search_hires_from_the_pool() {
    let mut c = Config::load();
    c.ambient.flats_per_slot = 4.0;
    c.world.population = 700;
    let mut w = World::new(42, c);
    w.run_ticks(4 * TICKS_PER_DAY + 1);
    let cap = w.config.per_capita_count(w.config.ambient.hires_per_day as usize);
    let hires: Vec<u32> = w.stats.history.iter().filter_map(|r| r.ambient.as_ref().map(|a| a.hires)).collect();
    println!("hires a day {hires:?} (cap {cap})");
    assert!(hires.iter().sum::<u32>() > 0, "a short city hires from the pool");
    assert!(hires.iter().all(|&h| h as usize <= cap));
}

/// A save mid-run resumes byte for byte with the tier's state.
#[test]
fn test_ambient_save_round_trip() {
    let mut w = ambient_world(42, 4.0);
    w.run_ticks(TICKS_PER_DAY + 300);
    for hi in [3, 9, 27] {
        ambient::materialise(&mut w, hi);
    }
    let text = save::to_ron(&w);
    assert!(text.contains("ambient"));
    let mut back = save::from_ron(&text).expect("loads");
    assert_eq!(save::to_ron(&back), text);
    w.run_ticks(TICKS_PER_DAY);
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(save::to_ron(&back), save::to_ron(&w));
}

/// One cohort's readings: (n, hunger, energy, mood, wallet median, contacts
/// median, employed share).
fn readings(w: &World, ids: &[EntityId]) -> (usize, f32, f32, f32, i64, usize, f32) {
    let n = ids.len();
    if n == 0 {
        return (0, 0.0, 0.0, 0.0, 0, 0, 0.0);
    }
    let mean = |f: &dyn Fn(EntityId) -> f32| ids.iter().map(|&i| f(i)).sum::<f32>() / n as f32;
    let hunger = mean(&|i| w.comp::<Needs>(i).map_or(0.0, |x| x.hunger));
    let energy = mean(&|i| w.comp::<Needs>(i).map_or(0.0, |x| x.energy));
    let mood = mean(&|i| w.comp::<Mood>(i).map_or(0.0, |x| x.value));
    let employed = mean(&|i| if w.has::<Job>(i) { 1.0 } else { 0.0 });
    let mut wallets: Vec<i64> = ids.iter().map(|&i| w.comp::<Wallet>(i).map_or(0, |x| x.coins)).collect();
    wallets.sort_unstable();
    let mut contacts: Vec<usize> = ids.iter().map(|&i| w.neighbours(i).count()).collect();
    contacts.sort_unstable();
    (n, hunger, energy, mood, wallets[n / 2], contacts[n / 2], employed)
}

/// The cohort against the born-simulated adults, by class: printed, and
/// loosely asserted where the cohort holds at least `min` adults.
fn compare(w: &World, cohort: &[EntityId], label: &str, min: usize) {
    let execs = classes::exec_set(w);
    let set: std::collections::BTreeSet<EntityId> = cohort.iter().copied().collect();
    // scan-ok: a test, once.
    let born: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&i| !set.contains(&i) && w.has::<Brain>(i) && citysim::systems::demography::is_adult(w, i))
        .collect();
    for class in citysim::Class::ALL {
        let m: Vec<EntityId> =
            cohort.iter().copied().filter(|&i| w.is_alive(i) && classes::class_in(w, i, &execs) == class).collect();
        let b: Vec<EntityId> = born.iter().copied().filter(|&i| classes::class_in(w, i, &execs) == class).collect();
        let (rm, rb) = (readings(w, &m), readings(w, &b));
        println!(
            "{label} {:<6} materialised n={:>3} hunger {:.2} energy {:.2} mood {:+.2} wallet {:>4} contacts {:>3} employed {:.2}",
            class.label(), rm.0, rm.1, rm.2, rm.3, rm.4, rm.5, rm.6
        );
        println!(
            "{label} {:<6} born         n={:>4} hunger {:.2} energy {:.2} mood {:+.2} wallet {:>4} contacts {:>3} employed {:.2}",
            class.label(), rb.0, rb.1, rb.2, rb.3, rb.4, rb.5, rb.6
        );
        if rm.0 < min || rb.0 < min {
            continue;
        }
        let what = format!("{label} {}", class.label());
        assert!((rm.1 - rb.1).abs() <= 0.25, "{what}: hunger {:.2} vs {:.2}", rm.1, rb.1);
        assert!((rm.2 - rb.2).abs() <= 0.25, "{what}: energy {:.2} vs {:.2}", rm.2, rb.2);
        assert!((rm.3 - rb.3).abs() <= 0.4, "{what}: mood {:.2} vs {:.2}", rm.3, rb.3);
        let (wm, wb) = (rm.4.max(1) as f64, rb.4.max(1) as f64);
        assert!(wm / wb <= 4.0 && wb / wm <= 4.0 || (rm.4 - rb.4).abs() <= 25, "{what}: wallet {} vs {}", rm.4, rb.4);
        let (cm, cb) = (rm.5.max(1) as f64, rb.5.max(1) as f64);
        assert!(cm / cb <= 3.0 && cb / cm <= 3.0, "{what}: contacts {} vs {}", rm.5, rb.5);
        assert!((rm.6 - rb.6).abs() <= 0.4, "{what}: employed {:.2} vs {:.2}", rm.6, rb.6);
    }
}

/// The parity-style check (docs/SCALE_20K.md § S5): on day 20 a household
/// from every fifth Ambient Home is materialised (a story touch); on day 20
/// (fresh) and day 30 the cohort's needs, wallet, contacts and employment are
/// printed against the born-simulated adults of the same class and asserted
/// in a loose band (a broken materialisation, not a calibration).
#[test]
fn test_materialised_look_like_born_residents() {
    let mut w = ambient_world(42, 4.0);
    w.run_ticks(20 * TICKS_PER_DAY + 9 * 60);
    let n = w.ambient.homes.len();
    let mut cohort = Vec::new();
    for hi in (0..n).step_by(5) {
        cohort.extend(ambient::materialise(&mut w, hi));
    }
    cohort.sort_unstable();
    println!("materialised {} adults from {} Homes", cohort.len(), n.div_ceil(5));
    assert!(cohort.len() >= 60);
    compare(&w, &cohort, "d20", 15);
    w.run_ticks(10 * TICKS_PER_DAY);
    let alive = cohort.iter().filter(|&&i| w.is_alive(i)).count();
    println!("d30: {alive} of {} alive", cohort.len());
    assert!(alive * 10 >= cohort.len() * 9, "the cohort died off: {alive} of {}", cohort.len());
    compare(&w, &cohort, "d30", 15);
}

/// Immigration draws from the Ambient flats first: with the outside wage
/// pinned low the week's pull brings arrivals (day 7's midnight), every one
/// an Ambient household (none walks in at the map edge while flats are left).
#[test]
fn test_immigration_draws_from_ambient_flats_first() {
    let mut w = ambient_world(42, 4.0);
    w.econ.outside_wage_pin = Some(0.1);
    w.run_ticks(7 * TICKS_PER_DAY + 1);
    let pull: u32 =
        w.stats.history.iter().map(|r| r.jobs.migrants_pull).sum::<u32>() + w.stats.current.jobs.migrants_pull;
    // Review finding 6: `immigrants` counts the Ambient arrivals too; an edge
    // arrival is an Immigration event for someone no flat materialised.
    let edge = w
        .events
        .iter()
        .filter(|e| e.kind == citysim::EventKind::Immigration)
        .filter(|e| e.actors.first().is_some_and(|id| w.ambient.materialised.binary_search(id).is_err()))
        .count();
    let arrivals = w.ambient.today.arrivals;
    println!("pull arrivals {pull}, Ambient households {arrivals}, edge immigrants {edge}");
    assert!(arrivals > 0, "the week's pull brought no Ambient household");
    assert_eq!(edge, 0, "an arrival walked in at the edge with Ambient flats left");
    let materialised = w.events.iter().filter(|e| e.kind == citysim::EventKind::Materialised).count();
    assert!(materialised as u32 >= arrivals);
}

/// Review finding 6: an Ambient household routed in by immigration (the
/// week's pull, an offer) is immigration: every adult counts in the day's
/// `immigrants` and gets an `Immigration` event (beside its `Materialised`).
/// A labour-pool hire is not (a resident of the city's flats taking a job).
#[test]
fn test_ambient_arrivals_count_as_immigrants() {
    let mut w = ambient_world(42, 4.0);
    w.econ.outside_wage_pin = Some(0.1);
    w.run_ticks(7 * TICKS_PER_DAY + 1);
    let pull: u32 =
        w.stats.history.iter().map(|r| r.jobs.migrants_pull).sum::<u32>() + w.stats.current.jobs.migrants_pull;
    let counted: u32 = w.stats.history.iter().map(|r| r.immigrants).sum::<u32>() + w.stats.current.immigrants;
    assert!(pull > 0, "the week's pull brought nobody");
    let arrived: Vec<EntityId> = w
        .events
        .iter()
        .filter(|e| e.kind == citysim::EventKind::Immigration)
        .filter_map(|e| e.actors.first().copied())
        .filter(|id| w.ambient.materialised.binary_search(id).is_ok())
        .collect();
    assert_eq!(arrived.len() as u32, pull, "one Immigration event per Ambient arrival");
    assert!(counted >= pull, "the day's immigrants count the Ambient arrivals ({counted} < {pull})");
}
