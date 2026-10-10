//! Person probes (docs/TESTING.md, the behaviour tier): one pinned agent in
//! the shipped seed-42 city, one stimulus, and the thing a person would do.
//! Each probe is a seeded run of a simulated resident's planner, read back
//! from its own state. A probe that exposes a gap is `#[ignore]`d with the
//! gap in a comment (the gap is a behaviour fix for a later round).

use citysim::contract::{Broker, ContractId, ContractKind, ContractStatus, Origin, Posting, Target};
use citysim::exec::routine;
use citysim::systems::{contracts, demography, law, leisure, social};
use citysim::{
    ActionKind, Brain, Building, Config, EntityId, Household, Inventory, Job, Needs, Position, RelKind, Role, Wallet,
    World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world() -> World {
    World::new(42, Config::load())
}

fn pin(w: &mut World, id: EntityId) {
    w.comp_mut::<Brain>(id).expect("brain").pinned = true;
}

/// Run to `tick`, the pin re-applied every tick (the shadow tool's rule).
fn run_to(w: &mut World, id: EntityId, tick: u64, mut each: impl FnMut(&World)) {
    while w.tick < tick {
        pin(w, id);
        w.run_ticks(1);
        each(w);
    }
}

fn step(w: &World, id: EntityId) -> Option<ActionKind> {
    w.comp::<Brain>(id).and_then(|b| b.current_step().map(|s| s.action))
}

fn is_eat(a: Option<ActionKind>) -> bool {
    matches!(a, Some(ActionKind::EatFromInventory | ActionKind::EatAtHome | ActionKind::EatOut | ActionKind::BuyFood))
}

fn coins(w: &World, id: EntityId) -> i64 {
    w.comp::<Wallet>(id).map_or(0, |x| x.coins)
}

fn home(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Household>(id).and_then(|h| h.home)
}

fn building(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Position>(id).and_then(|p| p.building)
}

/// Free housed civilian adults with a day job (not a guard, not a sweeper)
/// who are at their employer on shift right now.
fn day_workers_at_work(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && demography::is_adult(w, a))
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a) && home(w, a).is_some())
        .filter(|&a| {
            w.comp::<Job>(a).is_some_and(|j| {
                !matches!(j.role, Role::Guard | Role::Sanitation)
                    && j.shifts == vec![(540, 1080)]
                    && j.employer.is_some()
                    && j.employer == building(w, a)
                    && routine::workday_of(w, a, j, j.shift_key_at(w.tick))
            })
        })
        .collect()
}

/// Hungry, broke and with nothing in the larder at 15:00, two hours before a
/// shift ends: the wage lands at 18:00 and a person spends it on food before
/// midnight, when the rent is taken.
#[test]
fn probe_hungry_worker_eats_from_the_wage_before_rent() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 15 * TICKS_PER_HOUR);
    let a = *day_workers_at_work(&w).first().expect("a day worker at work at 15:00");
    pin(&mut w, a);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    w.comp_mut::<Inventory>(a).expect("inventory").food = 0;
    let h = home(&w, a).expect("a home");
    w.comp_mut::<Building>(h).expect("home").stock_food = 0;
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.25;
    let (mut paid_at, mut ate_after) = (None, false);
    let mut last = 0;
    run_to(&mut w, a, 2 * TICKS_PER_DAY, |w| {
        let c = coins(w, a);
        if c > last && paid_at.is_none() {
            paid_at = Some(w.tick);
        }
        last = c;
        if paid_at.is_some() && is_eat(step(w, a)) && !ate_after {
            ate_after = true;
            eprintln!("ate ({:?}) at day 1 {:02}:{:02}", step(w, a), (w.tick % TICKS_PER_DAY) / 60, w.tick % 60);
        }
    });
    let paid = paid_at.expect("the shift's wage was paid");
    eprintln!("paid at day 1 {:02}:{:02}, {} coins at midnight", (paid % TICKS_PER_DAY) / 60, paid % 60, coins(&w, a));
    assert!(
        ate_after,
        "paid at tick {paid} (day 1 {:02}:{:02}) but no food before midnight",
        (paid % TICKS_PER_DAY) / 60,
        paid % 60
    );
}

/// Across town at 20:00, tired, with a spouse: a person goes home to sleep.
#[test]
fn probe_married_adult_across_town_comes_home_at_night() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 20 * TICKS_PER_HOUR);
    let a = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && demography::is_adult(&w, a) && w.gang_of(a).is_none())
        .filter(|&a| !w.has::<citysim::Sentence>(a) && home(&w, a).is_some())
        .filter(|&a| w.comp::<Job>(a).is_none_or(|j| !j.on_shift(0) && !j.on_shift(180)))
        .find(|&a| w.spouses.contains_key(&a))
        .expect("a married housed adult");
    let h = home(&w, a).expect("home");
    let door = w.comp::<Building>(h).map(|b| b.door).expect("door");
    // A street tile 40-60 tiles off the door: across town, a walk of an hour or two.
    let far = (0..256u32)
        .flat_map(|x| (0..192u32).map(move |y| citysim::TilePos { x: x as u8, y: y as u8 }))
        .filter(|&t| (40..=60).contains(&t.manhattan(door)) && w.is_street(t))
        .min_by_key(|t| (t.manhattan(door), t.x, t.y))
        .expect("a street tile across town");
    w.leave_building(a);
    w.comp_mut::<Position>(a).expect("pos").tile = far;
    w.abort_plan(a);
    w.comp_mut::<Needs>(a).expect("needs").energy = 0.3;
    run_to(&mut w, a, 2 * TICKS_PER_DAY + 3 * TICKS_PER_HOUR, |_| {});
    let spouse = w.spouses.get(&a).copied();
    eprintln!(
        "at 03:00: in {:?} (home {h:?}); spouse shares the home: {}",
        building(&w, a),
        spouse.is_some_and(|s| home(&w, s) == Some(h))
    );
    assert_eq!(building(&w, a), Some(h), "at home at 03:00");
}

/// Two HangOut spots a few tiles apart; an Enemy stands at the nearer one.
/// A person picks the other corner.
#[test]
fn probe_enemy_at_the_spot_is_avoided() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 19 * TICKS_PER_HOUR);
    let civ = |w: &World, skip: &[EntityId]| -> EntityId {
        w.citizens()
            .into_iter()
            .filter(|&a| law::living(w, a) && demography::is_adult(w, a) && w.gang_of(a).is_none())
            .filter(|&a| !w.has::<citysim::Sentence>(a) && w.comp::<Job>(a).is_none() && !skip.contains(&a))
            .find(|&a| w.comp::<Brain>(a).is_some())
            .expect("a free civilian")
    };
    let a = civ(&w, &[]);
    let enemy = civ(&w, &[a]);
    // Two street tiles 6 tiles apart in one district; `a` stands 2 tiles from the first.
    let d = w.district_of(w.comp::<Position>(a).expect("pos").tile);
    let streets: Vec<citysim::TilePos> = (0..256u32)
        .flat_map(|x| (0..192u32).map(move |y| citysim::TilePos { x: x as u8, y: y as u8 }))
        .filter(|&t| w.is_street(t) && w.district_of(t) == d)
        .collect();
    let (near, other, from) = streets
        .iter()
        .find_map(|&p| {
            let q = streets.iter().copied().find(|&q| q.manhattan(p) == 6)?;
            let f = streets.iter().copied().find(|&f| f.manhattan(p) == 2 && f.manhattan(q) == 8)?;
            Some((p, q, f))
        })
        .expect("two spots and a start");
    let spots = vec![
        citysim::living::Spot { tile: near, kind: citysim::living::SpotKind::Barrel },
        citysim::living::Spot { tile: other, kind: citysim::living::SpotKind::Barrel },
    ];
    w.spots = vec![spots; w.districts.len()];
    w.leave_building(a);
    w.comp_mut::<Position>(a).expect("pos").tile = from;
    let (t0, _) = leisure::best_spot(&w, a, from).expect("a spot");
    assert_eq!(t0, near, "with nobody about, the nearer corner");
    w.edge_entry(a, enemy).affinity = -0.9;
    w.edge_entry(a, enemy).kind = RelKind::Enemy;
    social::reindex_kind(&mut w, a, enemy);
    w.leave_building(enemy);
    w.comp_mut::<Position>(enemy).expect("pos").tile = near;
    let (t1, _) = leisure::best_spot(&w, a, from).expect("a spot");
    assert_eq!(t1, other, "an Enemy on the nearer corner: the other one");
}

/// At work at 17:45 with the shift ending at 18:00 and a little hungry:
/// a person finishes the shift (and is paid) before going for food.
#[test]
fn probe_worker_finishes_the_shift_before_an_errand() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 17 * TICKS_PER_HOUR + 45);
    let a = day_workers_at_work(&w)
        .into_iter()
        .find(|&a| {
            matches!(
                step(&w, a),
                Some(ActionKind::ClerkWork | ActionKind::FabWork | ActionKind::FarmWork | ActionKind::BartendWork)
            )
        })
        .expect("a day worker at the counter at 17:45");
    let employer = building(&w, a).expect("at work");
    pin(&mut w, a);
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.3;
    let before = coins(&w, a);
    let mut left_early = None;
    run_to(&mut w, a, TICKS_PER_DAY + 18 * TICKS_PER_HOUR + 15, |w| {
        if w.tick < TICKS_PER_DAY + 18 * TICKS_PER_HOUR && building(w, a) != Some(employer) && left_early.is_none() {
            left_early = Some(w.tick % TICKS_PER_DAY);
        }
    });
    eprintln!("worker: {before} -> {} coins, left early {left_early:?}", coins(&w, a));
    assert_eq!(left_early, None, "left the workplace before 18:00");
    assert!(coins(&w, a) > before, "paid at the shift's end ({before} -> {})", coins(&w, a));
}

/// An immigrant arrives with nothing (no coins, no food): within five days a
/// person has found food (eaten) or work, and is alive.
#[test]
fn probe_broke_immigrant_finds_food_or_work() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY * 3);
    let a = demography::spawn_immigrant(&mut w);
    pin(&mut w, a);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    w.comp_mut::<Inventory>(a).expect("inventory").food = 0;
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.4;
    let (mut ate, mut hired) = (false, false);
    let end = w.tick + 5 * TICKS_PER_DAY;
    run_to(&mut w, a, end, |w| {
        ate |= law::living(w, a) && is_eat(step(w, a));
        hired |= w.comp::<Job>(a).is_some();
    });
    eprintln!("immigrant: ate {ate}, hired {hired}, hunger {:.2}", w.comp::<Needs>(a).map_or(0.0, |n| n.hunger));
    assert!(law::living(&w, a), "alive after five days");
    assert!(ate || hired, "neither ate nor found work in five days");
}

/// A guard on a long desk shift who starts it hungry: a person eats before
/// starving (hunger never reaches 0 over the twelve hours).
#[test]
fn probe_guard_eats_on_a_long_shift() {
    let mut w = world();
    w.run_ticks(TICKS_PER_DAY + 6 * TICKS_PER_HOUR);
    let g = w
        .guards()
        .iter()
        .copied()
        .find(|&g| law::living(&w, g) && !w.has::<citysim::Sentence>(g))
        .expect("a free guard");
    pin(&mut w, g);
    {
        let j = w.comp_mut::<Job>(g).expect("job");
        j.shifts = vec![(360, 1080)];
    }
    w.comp_mut::<Needs>(g).expect("needs").hunger = 0.3;
    if w.comp::<Wallet>(g).map_or(0, |x| x.coins) < 20 {
        w.comp_mut::<Wallet>(g).expect("wallet").coins = 20;
    }
    let (mut low, mut ate) = (1.0f32, false);
    run_to(&mut w, g, TICKS_PER_DAY + 18 * TICKS_PER_HOUR, |w| {
        low = low.min(w.comp::<Needs>(g).map_or(1.0, |n| n.hunger));
        ate |= is_eat(step(w, g));
    });
    eprintln!("guard: ate {ate}, lowest hunger {low:.2}");
    assert!(ate, "no meal in a twelve-hour shift started hungry (lowest hunger {low:.2})");
    assert!(low > 0.0, "starving on shift (hunger reached 0)");
}

/// Jobs and room P1 (spec § 7, plan P1 step 5): a Vat Tech laid off at
/// midnight in the wages + no-net city, with at least three posts open
/// city-wide, holds a Job again within 14 days (the rehire bonus and the
/// skill key put the laid-off hand back to work). Ignored until the flip
/// (J31): it needs `[economy2] wages` and `no_safety_net` on.
#[test]
#[ignore]
fn probe_laid_off_farm_worker_finds_work_within_14_days() {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    let mut w = World::new(42, cfg);
    // Day 20, a minute to midnight: the layoff lands just before the
    // midnight job search, as `wages::staff`'s do.
    w.run_ticks(21 * TICKS_PER_DAY - 1);
    let a = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .filter(|&a| law::living(&w, a) && !w.has::<citysim::Sentence>(a) && w.gang_of(a).is_none())
        .find(|&a| {
            w.comp::<Job>(a)
                .and_then(|j| j.employer)
                .and_then(|b| w.corp_of_building(b))
                .is_some_and(|c| w.comp::<citysim::Corp>(c).is_some_and(|cc| cc.exec != Some(a)))
        })
        .expect("a corp's Vat Tech");
    pin(&mut w, a);
    let farm = w.comp::<Job>(a).and_then(|j| j.employer);
    let text = format!("{} laid off as Vat Tech (probe)", w.name_of(a));
    citysim::systems::economy::dismiss_as(&mut w, a, farm, text, citysim::EventKind::LaidOff);
    assert!(!w.has::<Job>(a));
    let posts_open = w.vacancies.values().map(Vec::len).sum::<usize>();
    assert!(posts_open >= 3, "the stimulus needs three posts open city-wide ({posts_open})");
    let (mut hired_on, start) = (None, w.tick);
    run_to(&mut w, a, start + 14 * TICKS_PER_DAY, |w| {
        if hired_on.is_none() && w.has::<Job>(a) {
            hired_on = Some((w.tick - start) / TICKS_PER_DAY);
        }
    });
    let job = w.comp::<Job>(a).map(|j| (j.role.label(), j.employer));
    eprintln!("laid off on day 20: hired after {hired_on:?} days as {job:?} (posts open at the search: {posts_open})");
    assert!(law::living(&w, a), "alive");
    assert!(hired_on.is_some(), "no Job within 14 days of the layoff");
}

/// Jobs and room P6 (spec § 7, "the offer is real"): a migrant spawned with
/// an offer at the weekly midnight is at that workplace on shift within two
/// days. Ignored until the flip (J31): it needs `[economy2] wages` and
/// `no_safety_net` on.
#[test]
#[ignore]
fn probe_offer_migrant_reaches_the_first_shift() {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    cfg.demography.p_spouse = 0.0;
    let mut w = World::new(42, cfg);
    // Day 14's midnight, when the week's offers spawn.
    w.run_ticks(14 * TICKS_PER_DAY);
    // A Vat Farm's post open five days (the stimulus; the city's own posts
    // are mostly filled within the day).
    let farm = w
        .buildings_of_kind(citysim::BuildingKind::Farm)
        .iter()
        .copied()
        .find(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .expect("a Farm");
    w.vacancies.entry(farm).or_default().push(Role::Farmer);
    let since = w.tick - 5 * TICKS_PER_DAY;
    w.econ.vacancy_since.insert((farm, Role::Farmer), since);
    let a = demography::spawn_immigrant_for(&mut w, farm, Role::Farmer).expect("an offer migrant");
    pin(&mut w, a);
    let mut first = None;
    let start = w.tick;
    run_to(&mut w, a, start + 2 * TICKS_PER_DAY, |w| {
        if first.is_some() {
            return;
        }
        let Some(j) = w.comp::<Job>(a) else { return };
        let tod = (w.tick % TICKS_PER_DAY) as u16;
        let on_shift = j.shifts.iter().any(|&(s, e)| (s..e).contains(&tod))
            && routine::workday_of(w, a, j, j.shift_key_at(w.tick));
        if on_shift && building(w, a) == Some(farm) {
            first = Some(w.tick - start);
        }
    });
    eprintln!(
        "offer migrant: at the Farm on shift after {:?} ticks; job {:?}",
        first,
        w.comp::<Job>(a).map(|j| j.role.label())
    );
    assert!(law::living(&w, a), "alive");
    assert!(first.is_some(), "never at the workplace on shift within two days");
}

/// Jobs and room P3 (spec § 7, plan P3 step 4): a Night Porter on her shift
/// is at the Hotel at 02:00 and has been paid by 08:00 (the night shift
/// ends at 06:00 and the wage is paid at its end, from the Hotel's owner).
#[test]
fn probe_night_porter_on_shift_at_two_is_paid_by_eight() {
    let mut w = world();
    let porter_role = w.config.trade_role("night_porter").expect("the night_porter trade is configured");
    // Day 2, 00:00: the first midnight's job search hired the porters.
    w.run_ticks(2 * TICKS_PER_DAY);
    let p = w
        .workers(porter_role)
        .iter()
        .copied()
        .find(|&a| {
            law::living(&w, a)
                && !w.has::<citysim::Sentence>(a)
                && w.comp::<Job>(a).is_some_and(|j| routine::workday_of(&w, a, j, j.shift_key_at(w.tick)))
        })
        .expect("a free Night Porter on a workday");
    let hotel = w.comp::<Job>(p).and_then(|j| j.employer).expect("employer");
    pin(&mut w, p);
    run_to(&mut w, p, 2 * TICKS_PER_DAY + 2 * TICKS_PER_HOUR, |_| {});
    let at_two = building(&w, p);
    let before = coins(&w, p);
    let mut paid = false;
    let mut last = before;
    run_to(&mut w, p, 2 * TICKS_PER_DAY + 8 * TICKS_PER_HOUR, |w| {
        let c = coins(w, p);
        paid |= c > last;
        last = c;
    });
    let unpaid = w.comp::<Job>(p).map_or(99, |j| j.days_unpaid);
    eprintln!(
        "porter {}: at 02:00 in {:?} (Hotel {:?}); coins {before} -> {}; paid {paid}; days_unpaid {unpaid}",
        p.index,
        at_two.map(|b| b.index),
        hotel.index,
        coins(&w, p)
    );
    assert_eq!(at_two, Some(hotel), "the porter is not at the Hotel at 02:00");
    assert!(paid && unpaid == 0, "the night shift was not paid by 08:00");
}

// ---------------------------------------------------------------------------
// M16a phase 5 (5.1 c): the contract board's person probes. A contract is a
// record with a price on a struct, matched by a score and resolved by one
// seeded roll; the probes read one pinned agent's own state.
// ---------------------------------------------------------------------------

/// Free civilian adults (no gang, no guard's job, not jailed), ascending.
fn civilians(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && demography::is_adult(w, a) && w.comp::<Brain>(a).is_some())
        .filter(|&a| w.gang_of(a).is_none() && !law::is_guard(w, a) && !w.has::<citysim::Sentence>(a))
        .collect()
}

/// A brokered Hit by `buyer` (also the placing agent) on `t` at the first open
/// Fixer, priced at `price`.
fn post_hit(w: &mut World, buyer: EntityId, t: EntityId, price: i64) -> ContractId {
    let f = contracts::open_fixers(w)[0];
    w.comp_mut::<Wallet>(buyer).expect("wallet").coins = price + 100;
    let p = Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind: ContractKind::Hit,
        target: Target::Agent(t),
        broker: Some(f),
        deadline_days: 10,
        origin: Origin::God,
        price: Some(price),
    };
    contracts::post(w, p).expect("posted")
}

/// Make `id` the sort who takes a Hit: lawless, brave, a strong fighter,
/// jobless, and (alone) a regular of the Fixer.
fn make_gun(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<citysim::Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<citysim::Skills>(id) {
        s.fighting = 1.0;
        s.stealth = 1.0;
    }
    w.vacate_job(id);
    let f = contracts::open_fixers(w)[0];
    let now = w.tick;
    if let Some(k) = w.comp_mut::<Broker>(f) {
        k.regulars.clear();
        k.regulars.insert(id, now);
    }
}

/// Did `who` ever hold record `c` (its taker or crew)?
fn holds(w: &World, who: EntityId, c: ContractId) -> bool {
    w.contracts.get(&c).is_some_and(|x| x.taker == Some(who) || x.crew.contains(&who))
}

/// A gun, alone on the Fixer's books, sees a brokered Hit on his own wife at
/// a price far above a month's wage: a person never takes it. Control: the
/// same gun takes the same Hit on a stranger (the setup is one he would take).
#[test]
fn probe_gun_never_takes_a_hit_on_kin() {
    let run = |kin: bool| -> (bool, Option<ContractStatus>) {
        let mut w = world();
        w.run_ticks(2 * TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
        let civ = civilians(&w);
        let a = civ
            .iter()
            .copied()
            .find(|&a| w.spouses.get(&a).is_some_and(|s| civ.contains(s)))
            .expect("a married civilian");
        let wife = w.spouses[&a];
        let t = if kin {
            wife
        } else {
            civ.iter().copied().find(|&x| x != a && x != wife && w.edge(a, x).is_none()).expect("a stranger")
        };
        let buyer = civ
            .iter()
            .copied()
            .find(|&x| ![a, wife, t].contains(&x) && w.edge(x, a).is_none() && w.edge(x, t).is_none())
            .expect("a buyer");
        pin(&mut w, a);
        make_gun(&mut w, a);
        let id = post_hit(&mut w, buyer, t, 2000);
        let mut took = false;
        let end = w.tick + 3 * TICKS_PER_DAY;
        run_to(&mut w, a, end, |w| took |= holds(w, a, id));
        let status = w.contracts.get(&id).map(|c| c.status);
        eprintln!(
            "gun {} on {} ({}): took {took}, record {status:?}",
            a.index,
            t.index,
            if kin { "wife" } else { "stranger" }
        );
        (took, status)
    };
    let (control, _) = run(false);
    assert!(control, "the control: the gun takes the same Hit on a stranger (the probe's setup)");
    let (took, _) = run(true);
    assert!(!took, "the gun took the Hit on his own wife");
}

/// A buyer places a brokered Hit, a gun takes it and the target dies by his
/// hand (day 2, 12:00); then nobody but the buyer holds the `Hired` deed
/// (the stimulus: every other memory of it cleared, the broker's and the
/// gun's included). Returns the world and (buyer, target, gun).
fn quiet_buyer() -> (World, EntityId, EntityId, EntityId) {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
    let civ = civilians(&w);
    let buyer = civ[0];
    let t = civ.iter().copied().find(|&x| x != buyer && w.edge(buyer, x).is_none()).expect("a target");
    let taker = civ.iter().copied().find(|&x| ![buyer, t].contains(&x) && w.edge(x, t).is_none()).expect("a taker");
    pin(&mut w, buyer);
    make_gun(&mut w, taker);
    let id = post_hit(&mut w, buyer, t, 1000);
    contracts::god_take(&mut w, id, taker).expect("taken");
    w.kill_by(t, citysim::DeathCause::Violence, Some(taker));
    if w.contracts[&id].status != ContractStatus::Fulfilled {
        contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    }
    assert_eq!(w.contracts[&id].status, ContractStatus::Fulfilled);
    for who in w.with::<citysim::Memory>() {
        if who == buyer {
            continue;
        }
        if let Some(m) = w.comp_mut::<citysim::Memory>(who) {
            let keep = |e: &citysim::MemoryEntry| !is_hired(e, buyer, t);
            m.entries.retain(keep);
            m.heard.retain(keep);
        }
    }
    (w, buyer, t, taker)
}

fn is_hired(e: &citysim::MemoryEntry, buyer: EntityId, t: EntityId) -> bool {
    e.deed == Some(citysim::word::Deed::Hired) && e.subject == Some(buyer) && e.object == Some(t)
}

/// Everyone but `buyer` holding the `Hired` deed about (buyer, t).
fn heard_of(w: &World, buyer: EntityId, t: EntityId) -> Vec<EntityId> {
    w.with::<citysim::Memory>()
        .into_iter()
        .filter(|&h| h != buyer)
        .filter(|&h| {
            w.comp::<citysim::Memory>(h)
                .is_some_and(|m| m.entries.iter().chain(m.heard.iter()).any(|e| is_hired(e, buyer, t)))
        })
        .collect()
}

/// The buyer of a fulfilled Hit whom nobody has heard of: for 30 days the
/// law never charges him before somebody (a guard, a witness, an
/// interrogated gun) holds the deed (checked at each day's end).
#[test]
fn probe_buyer_nobody_heard_of_is_never_charged() {
    let (mut w, buyer, t, taker) = quiet_buyer();
    let charged =
        |w: &World| w.crime_reports().iter().any(|r| r.crime == citysim::Crime::Conspiracy && r.suspect == buyer);
    let (mut heard_on, mut charged_on) = (None, None);
    let start = w.tick;
    for day in 0..30u64 {
        run_to(&mut w, buyer, start + (day + 1) * TICKS_PER_DAY, |_| {});
        if heard_on.is_none() {
            let hs = heard_of(&w, buyer, t);
            if !hs.is_empty() {
                let guards = hs.iter().filter(|&&h| law::is_guard(&w, h)).count();
                eprintln!("day {day}: {} hold the deed ({guards} guards)", hs.len());
                heard_on = Some(day);
            }
        }
        if charged_on.is_none() && charged(&w) {
            charged_on = Some(day);
        }
    }
    eprintln!(
        "buyer {}: heard of on day {heard_on:?}, charged on day {charged_on:?}; the gun jailed {}",
        buyer.index,
        w.has::<citysim::Sentence>(taker)
    );
    if let Some(c) = charged_on {
        let h = heard_on.expect("charged though nobody but the buyer ever held the deed");
        assert!(h <= c, "charged on day {c} before anybody heard of him (day {h})");
    }
}

/// GAP (M16a phase 5; for the behaviour round): the buyer tells his own
/// Hit. `gossip::exchange_one` lets a teller pass on a deed whose actor he
/// is (W8: a lie at `deception × 0.5`, else the truth), and `Hired`'s
/// salience makes it his best story, so within the day two city guards
/// hold it from him and the next midnight's `accessory_check` charges him
/// (seed 42: heard on day 0, charged on day 1). A person keeps a bought
/// killing to himself, above all from the law.
#[test]
#[ignore]
fn probe_buyer_keeps_his_own_hit_quiet() {
    let (mut w, buyer, t, _) = quiet_buyer();
    let end = w.tick + 7 * TICKS_PER_DAY;
    run_to(&mut w, buyer, end, |_| {});
    let guards: Vec<u32> =
        heard_of(&w, buyer, t).into_iter().filter(|&h| law::is_guard(&w, h)).map(|h| h.index).collect();
    eprintln!("guards holding the buyer's own deed after 7 days: {guards:?}");
    assert!(guards.is_empty(), "the buyer told the law of his own Hit");
}

/// A jobless agent with a skill a Fixer can sell, broke (no coins: the
/// wealth need at 0), not on any Fixer's books: within a week a person
/// goes to the Fixer and becomes a regular.
#[test]
fn probe_broke_hand_networks_at_a_fixer_within_a_week() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 9 * TICKS_PER_HOUR);
    let f = contracts::open_fixers(&w)[0];
    let regular = |w: &World, a: EntityId| w.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&a));
    let a = civilians(&w)
        .into_iter()
        .filter(|&a| home(&w, a).is_some() && !regular(&w, a))
        .filter(|&a| w.comp::<citysim::Personality>(a).is_some_and(|p| p.lawfulness < 0.5))
        .find(|&a| contracts::gun_gate(&w, a) && contracts::nearest_fixer(&w, a) == Some(f))
        .expect("a jobless hand with a sellable skill near the Fixer");
    pin(&mut w, a);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    w.comp_mut::<Needs>(a).expect("needs").wealth = 0.0;
    let (mut on, start) = (None, w.tick);
    run_to(&mut w, a, start + 7 * TICKS_PER_DAY, |w| {
        if on.is_none() && regular(w, a) {
            on = Some(w.tick - start);
        }
    });
    eprintln!(
        "hand {}: a regular after {:?} ticks (lawfulness {:.2})",
        a.index,
        on,
        w.comp::<citysim::Personality>(a).map_or(0.0, |p| p.lawfulness)
    );
    assert!(on.is_some(), "never networked at the Fixer within a week of going broke");
}
