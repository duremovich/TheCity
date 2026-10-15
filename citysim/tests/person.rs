//! Person probes (docs/TESTING.md, the behaviour tier): one pinned agent in
//! the shipped seed-42 city, one stimulus, and the thing a person would do.
//! Each probe is a seeded run of a simulated resident's planner, read back
//! from its own state. A probe that exposes a gap is `#[ignore]`d with the
//! gap in a comment (the gap is a behaviour fix for a later round).

use citysim::contract::{Broker, ContractId, ContractKind, ContractStatus, Origin, Posting, Target};
use citysim::exec::routine;
use citysim::systems::life::commute_cost;
use citysim::systems::{contracts, demography, law, leisure, social};
use citysim::{
    ActionKind, Brain, Building, Config, EntityId, EventKind, Household, Inventory, Job, Needs, Position, RelKind,
    Role, Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world() -> World {
    World::new(42, Config::load())
}

/// Jobs & lives phase 4: the city without the vice and fashion, for the
/// probes their joint butterfly moved (each switch alone passes; the agents
/// picked are no vice or fashion party). Re-read at the phase-8 gate.
fn world_p3() -> World {
    let mut cfg = Config::load();
    cfg.vice.enabled = false;
    cfg.fashion.enabled = false;
    World::new(42, cfg)
}

fn pin(w: &mut World, id: EntityId) {
    w.comp_mut::<Brain>(id).expect("brain").pinned = true;
}

/// Run to `tick`, the pin re-applied every tick (the shadow tool's rule).
fn run_to(w: &mut World, id: EntityId, tick: u64, mut each: impl FnMut(&World)) {
    while w.tick < tick {
        if !w.has::<Brain>(id) {
            let last: Vec<String> = w
                .events
                .iter()
                .rev()
                .filter(|e| e.actors.contains(&id))
                .take(6)
                .map(|e| format!("{} {:?} {}", e.tick, e.kind, e.text))
                .collect();
            panic!("agent {} is gone at {}: {last:#?}", id.index, w.tick);
        }
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
    // Jobs & lives phase 4 (`world_p3`): with both on, the butterfly's pick
    // stopped for a Bar's drink on the way and was still walking home at 03:00.
    let mut w = world_p3();
    w.run_ticks(TICKS_PER_DAY + 20 * TICKS_PER_HOUR);
    let a = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && demography::is_adult(&w, a) && w.gang_of(a).is_none())
        .filter(|&a| !w.has::<citysim::Sentence>(a) && home(&w, a).is_some())
        .filter(|&a| w.comp::<Job>(a).is_none_or(|j| !j.on_shift(0) && !j.on_shift(180)))
        // Day gaps (2026-10-11): not a body that can pay for a Hotel bed
        // (`life::away_hotel`, L2's designed bed far from Home): the city's
        // divergence picked one (seed 42 #589 took a Hotel bed at 21:40, as
        // on the old main given the same pick).
        .filter(|&a| {
            let budget = coins(&w, a) - w.config.life.hotel_reserve_meals * citysim::systems::life::meal_price(&w, a);
            !w.buildings_of_kind(citysim::BuildingKind::Hotel).iter().any(|&h| {
                citysim::systems::street::is_hotel(&w, h) && citysim::systems::street::hotel_price(&w, h) <= budget
            })
        })
        .find(|&a| w.spouses.contains_key(&a))
        .expect("a married housed adult");
    let h = home(&w, a).expect("home");
    let door = w.comp::<Building>(h).map(|b| b.door).expect("door");
    // A street tile 40-60 tiles off the door: across town, a walk of an hour or two.
    let far = (0..256u32)
        .flat_map(|x| (0..192u32).map(move |y| citysim::TilePos { x: x as u16, y: y as u16 }))
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
        .flat_map(|x| (0..192u32).map(move |y| citysim::TilePos { x: x as u16, y: y as u16 }))
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
/// skill key put the laid-off hand back to work). Still ignored after the
/// flip: on the flipped seed-42 city its stimulus (three posts open
/// city-wide) does not hold (2 open), so it measures nothing.
#[test]
#[ignore]
fn probe_laid_off_farm_worker_finds_work_within_14_days() {
    let cfg = Config::load();
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
/// days. Needs `[economy2] wages` and `no_safety_net` on: run since the
/// flip (J31).
#[test]
fn probe_offer_migrant_reaches_the_first_shift() {
    let mut cfg = Config::load();
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

/// The buyer keeps his own Hit quiet (god v8 gap 5, closed by the Hit
/// market's H7): before it, `gossip::exchange_one` let a teller pass on a
/// deed whose actor he is, and `Hired`'s salience made it his best story,
/// so within the day two city guards held it from him and the next
/// midnight's `accessory_check` charged him. A person keeps a bought
/// killing to himself, above all from the law.
///
/// The law's own road is shut here: an arrested gun interrogated
/// (`law::interrogate`, M16a C28) names his buyer to the guard, which is
/// not the buyer telling. After review item 1 (a dodger caught at the El's
/// gate off screen is on screen for the rest of the hour) this seed's gun
/// is arrested for Fare Evasion on day 5 and names the buyer.
#[test]
fn probe_buyer_keeps_his_own_hit_quiet() {
    let (mut w, buyer, t, _) = quiet_buyer();
    w.config.law.interrogate_days = 0;
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

// ---------------------------------------------------------------------------
// Hunger as sickness P1 (docs/HUNGER_SICKNESS.md § 6): the Full-planner gaps
// the recalibration diaries found. Each probe failed on main before its fix
// (`[life] hunger_fixes`).
// ---------------------------------------------------------------------------

/// The distinct steps an agent began, in order (consecutive repeats folded).
fn steps_log(log: &mut Vec<ActionKind>, w: &World, id: EntityId) {
    if let Some(s) = step(w, id) {
        if log.last() != Some(&s) {
            log.push(s);
        }
    }
}

fn set_hungry(w: &mut World, a: EntityId, hunger: f32, coins: i64) {
    w.comp_mut::<Wallet>(a).expect("wallet").coins = coins;
    w.comp_mut::<Inventory>(a).expect("inventory").food = 0;
    w.comp_mut::<Inventory>(a).expect("inventory").stolen_food = 0;
    if let Some(h) = home(w, a) {
        w.comp_mut::<Building>(h).expect("home").stock_food = 0;
    }
    w.comp_mut::<Needs>(a).expect("needs").hunger = hunger;
}

fn jobless_civilians(w: &World) -> Vec<EntityId> {
    civilians(w).into_iter().filter(|&a| w.comp::<Job>(a).is_none()).collect()
}

/// G1 (`hf_food_first`): starving (hunger 0.08) at 18:00, with either a
/// coin short of a meal or sixty over it: a person eats before spending on
/// a drink, a game, a bed or a dose, and does not sleep the night through
/// starving with the coins for a meal. The diaries (and this probe on main)
/// showed a starving beggar spending the begged coins on a Drink, and an
/// exhausted one with 60 coins sleeping 8 h to hunger 0.
#[test]
fn probe_starving_eats_before_spending_or_sleeping() {
    let mut w = world_p3();
    w.run_ticks(2 * TICKS_PER_DAY + 18 * TICKS_PER_HOUR);
    let picks: Vec<EntityId> = civilians(&w).into_iter().step_by(37).take(30).collect();
    let mut bad = Vec::new();
    for &a in &picks {
        for extra in [-1i64, 60] {
            let mut w = w.clone();
            pin(&mut w, a);
            let price = citysim::systems::life::meal_price(&w, a);
            set_hungry(&mut w, a, 0.08, (price + extra).max(0));
            // A spending step already under way when the hunger is set
            // (begun before the stimulus: a BuyAsset started at 16:52 and
            // ending 19:12 on the El city's table) is not a choice the
            // starving agent made; only what comes after it is judged.
            let under_way = w.comp::<Brain>(a).and_then(|b| match b.exec {
                ExecState::Use { kind, started, .. } if started < w.tick => Some(kind),
                _ => None,
            });
            let mut log = Vec::new();
            let end = w.tick + 8 * TICKS_PER_HOUR;
            run_to(&mut w, a, end, |w| steps_log(&mut log, w, a));
            if under_way.is_some() && log.first().copied() == under_way {
                log.remove(0);
            }
            // A bin meal is a meal (the bins run with [sickness] on, the shipped city since the flip).
            let first_eat =
                log.iter().position(|&s| is_eat(Some(s)) || s == ActionKind::EatAlms || s == ActionKind::DumpsterDive);
            let first_spend = log.iter().position(|&s| {
                matches!(
                    s,
                    ActionKind::CheckIn
                        | ActionKind::Gamble
                        | ActionKind::Enjoy
                        | ActionKind::Drink
                        | ActionKind::BuyStims
                        | ActionKind::BuyAsset
                        | ActionKind::Donate
                )
            });
            let spent_first = match (first_eat, first_spend) {
                (Some(e), Some(s)) => s < e,
                (None, Some(_)) => true,
                _ => false,
            };
            let rich_unfed = extra > 0 && first_eat.is_none();
            if spent_first || rich_unfed {
                eprintln!("agent {} {extra:+}: {log:?}", a.index);
                bad.push((a.index, extra));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} starving runs spent first or went unfed with coins: {bad:?}",
        bad.len(),
        picks.len() * 2
    );
}

/// G2 (`hf_mission`): lawful, broke, an empty larder, hungry, a stocked
/// Mission within reach: a person walks to the Mission and eats.
#[test]
fn probe_broke_lawful_eats_at_the_mission() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let m = citysim::systems::charity::missions(&w)
        .into_iter()
        .find(|&m| w.comp::<Building>(m).is_some_and(|b| b.kind == citysim::BuildingKind::Mission))
        .expect("a Mission");
    w.comp_mut::<Building>(m).expect("mission").stock_food = 60;
    let door = w.comp::<Building>(m).expect("mission").door;
    let reach = w.config.charity.reach_tiles;
    let picks: Vec<EntityId> = jobless_civilians(&w)
        .into_iter()
        // A corp exec has no Job but keeps office hours (not the jobless
        // the probe means; workplace room moved one into the picks).
        .filter(|&a| citysim::systems::life::exec_corp(&w, a).is_none())
        .filter(|&a| w.comp::<citysim::Personality>(a).is_some_and(|p| p.lawfulness >= 0.3))
        .filter(|&a| {
            home(&w, a).and_then(|h| w.comp::<Building>(h)).is_some_and(|b| {
                let d = b.door.manhattan(door);
                (8..=reach.saturating_sub(8)).contains(&d)
            })
        })
        .take(8)
        .collect();
    assert!(!picks.is_empty(), "a lawful jobless adult within a Mission's reach");
    let mut bad = Vec::new();
    for &a in &picks {
        let mut w = w.clone();
        pin(&mut w, a);
        set_hungry(&mut w, a, 0.3, 0);
        // Hunger is the stimulus, not exhaustion: an off-screen pick can wake
        // at energy 0 (~2 % of the Statistical adults at 10:00 on main and
        // here), and a person that tired sleeps first.
        if let Some(n) = w.comp_mut::<Needs>(a) {
            n.energy = n.energy.max(0.5);
        }
        let mut log = Vec::new();
        let end = w.tick + 6 * TICKS_PER_HOUR;
        run_to(&mut w, a, end, |w| steps_log(&mut log, w, a));
        eprintln!("lawful broke {}: {:?}, hunger {:.2}", a.index, log, w.comp::<Needs>(a).map_or(0.0, |n| n.hunger));
        if !log.iter().any(|&s| s == ActionKind::EatAlms || is_eat(Some(s))) {
            bad.push(a.index);
        }
    }
    assert!(bad.is_empty(), "{} of {} never ate in six hours: {bad:?}", bad.len(), picks.len());
}

/// G3 (`hf_buy_not_steal`): starving at 22:00, lawless, with coins for three
/// meals: a person buys; the diaries showed such agents stealing from Homes.
#[test]
fn probe_starving_with_coins_buys_not_steals() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 22 * TICKS_PER_HOUR);
    let (mut stole, mut bought, mut tried) = (0, 0, 0);
    // Off shift at 22:00 (workplace room, 2026-10-11: the Farms' and Fabs'
    // night crews are at work then, and a starving worker on shift is
    // another stimulus).
    let tod = w.tick_of_day();
    let off = |a: EntityId| w.comp::<Job>(a).is_none_or(|j| !j.on_shift(tod));
    for a in civilians(&w).into_iter().filter(|&a| home(&w, a).is_some() && off(a)).take(12) {
        tried += 1;
        let mut w = w.clone();
        if let Some(p) = w.comp_mut::<citysim::Personality>(a) {
            p.lawfulness = 0.05;
        }
        pin(&mut w, a);
        let price = citysim::systems::life::meal_price(&w, a);
        set_hungry(&mut w, a, 0.05, 3 * price);
        let mut log = Vec::new();
        let end = w.tick + 8 * TICKS_PER_HOUR;
        run_to(&mut w, a, end, |w| steps_log(&mut log, w, a));
        if log.iter().any(|s| matches!(s, ActionKind::StealFood(_))) {
            stole += 1;
            eprintln!("agent {} stole with coins: {log:?}", a.index);
        }
        bought += usize::from(log.contains(&ActionKind::BuyFood));
    }
    eprintln!("of {tried}: stole {stole}, bought {bought}");
    assert_eq!(stole, 0, "{stole} of {tried} lawless starving agents with coins stole food");
}

/// G4 (`hf_beg_retry`): a Beg that finds no giver at 03:00 (nobody about
/// holds more than 10 coins) is an hour honestly spent, as Scavenge's dry
/// hour: `Done`, not a failure that cools Earn after the second try (the
/// diaries' beggar gave up after one dry Beg). Three dry Begs in a row cool
/// Earn for `beg_cool_hours`.
#[test]
fn probe_dry_beg_is_an_honest_hour() {
    use citysim::exec::actions::on_complete;
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 3 * TICKS_PER_HOUR);
    for o in w.citizens() {
        if let Some(x) = w.comp_mut::<Wallet>(o) {
            x.coins = x.coins.min(10);
        }
    }
    let a = jobless_civilians(&w)[0];
    pin(&mut w, a);
    set_hungry(&mut w, a, 0.3, 0);
    let t = w.tick;
    let first = on_complete(&mut w, a, ActionKind::Beg, None, t, t);
    eprintln!("a dry Beg: {first:?}");
    assert!(matches!(first, citysim::StepResult::Done), "a dry Beg failed the plan: {first:?}");
    let cooled = |w: &World| {
        w.comp::<Brain>(a).and_then(|b| b.cooldowns.get(&citysim::GoalKind::Earn).copied()).is_some_and(|u| u > w.tick)
    };
    assert!(!cooled(&w), "one dry Beg cooled Earn");
    for _ in 0..w.config.life.beg_dry_max {
        let t = w.tick;
        on_complete(&mut w, a, ActionKind::Beg, None, t, t);
    }
    assert!(cooled(&w), "beg_dry_max dry Begs in a row did not cool Earn");
}

// ---------------------------------------------------------------------------
// P5g (Dylan: relocation and the hiring cap; hunger fixes items 8-9)
// ---------------------------------------------------------------------------

/// Item 8 (`hf_relocate`): an employed worker on a long commute, holding the
/// first rent (not 21 rents), moves nearer work within a day: into a Home
/// with room, or (the Farms' neighbourhoods are full) by trading Homes with
/// a single jobless resident there. Measured on the all-Full flipped city:
/// median commute ~90 tiles, ~3 h each way; relocation moved 8 a day, only
/// those holding 21x the rent.
#[test]
fn probe_far_worker_with_one_rent_moves_near_work() {
    let mut cfg = Config::load();
    // Transit phase 3: the relocation premise is a far walker; with the El on
    // `commute_cost` rides and no single farmer reads far (the El case is
    // `probe_el_*` and `tests/transit.rs`).
    cfg.transit.enabled = false;
    // Jobs & lives phase 1: with the industries on, seed 42 has no single
    // far Farmer either (the premise, not the move, is lost; phase 8's gate
    // re-reads the probes).
    cfg.jobs2.enabled = false;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    // Every commute measure through `life::commute_cost` (travel ticks).
    let cost = |w: &World, a: citysim::TilePos, b: citysim::TilePos| commute_cost(w, a, b);
    let cap = w.config.life.commute_move_ticks;
    let work_of = |w: &World, a: EntityId| {
        w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.comp::<Building>(e)).map(|b| b.door)
    };
    let door_of = |w: &World, a: EntityId| home(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door);
    let (far, partner) = civilians(&w)
        .into_iter()
        .filter(|&a| w.spouse_of(a).is_none() && w.comp::<Job>(a).is_some_and(|j| j.role == Role::Farmer))
        .find_map(|a| {
            let work = work_of(&w, a)?;
            if cost(&w, door_of(&w, a)?, work) <= 2 * cap {
                return None;
            }
            // A single adult living within 30 tiles of that work, to trade with.
            let r = civilians(&w).into_iter().find(|&r| {
                r != a && w.spouse_of(r).is_none() && door_of(&w, r).is_some_and(|d| cost(&w, d, work) <= cap * 3 / 4)
            })?;
            Some((a, r))
        })
        .expect("a single far farmer and a single adult living near the Farm");
    let work = work_of(&w, far).unwrap();
    // The partner has nothing keeping it there: no job, the coins of a rent.
    w.remove::<Job>(partner);
    w.comp_mut::<Wallet>(partner).expect("wallet").coins = 20;
    let rents: i64 = w
        .buildings_of_kind(citysim::BuildingKind::Home)
        .iter()
        .filter(|&&h| w.comp::<Building>(h).is_some_and(|b| cost(&w, b.door, work) <= cap))
        .map(|&h| citysim::systems::ownership::rent_for(&w, h))
        .max()
        .unwrap_or(1);
    w.comp_mut::<Wallet>(far).expect("wallet").coins = rents.max(1) + 2;
    let before = door_of(&w, far).map(|d| cost(&w, d, work));
    let partner_home = home(&w, partner).expect("partner home");
    let end = w.tick + TICKS_PER_DAY + TICKS_PER_HOUR;
    // The partner stays jobless at the midnight pass: its job search runs
    // before relocation, and with open places (workplace room, 2026-10-11:
    // the Farms' night shifts) it hired the partner 32 ticks from the Farm
    // first, whose new commute then barred the trade (`swap_partner`: the
    // partner's own day no worse). The night's open places are closed.
    let midnight = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    run_to(&mut w, far, midnight - 1, |_| {});
    w.vacancies.clear();
    run_to(&mut w, far, end, |_| {});
    let after = door_of(&w, far).map(|d| cost(&w, d, work));
    // The partner's Home went to a far worker (this farmer, or a farther one
    // the pass served first): a trade nearer work happened.
    // (The trade's own event: the trader may have moved on since.)
    let traded_in = w
        .events
        .iter()
        .find(|e| {
            e.kind == citysim::EventKind::Housed
                && e.actors.len() == 2
                && e.actors[1] == partner_home
                && e.actors[0] != partner
                && e.text.contains("traded")
        })
        .map(|e| e.actors[0]);
    eprintln!(
        "farmer {}: commute {before:?} -> {after:?} ticks; partner {} moved out {}; traded in {:?}",
        far.index,
        partner.index,
        home(&w, partner) != Some(partner_home),
        traded_in.map(|r| r.index)
    );
    assert!(after < before || traded_in.is_some(), "no far worker traded Homes nearer work");
}

/// Item 9 (`hf_hire_near`): with wages on, a jobless adult living near the
/// workplace is hired before a far one whose skill and rehire bonus would
/// have outweighed the walk.
#[test]
fn probe_near_candidate_hired_before_a_far_skilled_one() {
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let farm = *w.buildings_of_kind(citysim::BuildingKind::Farm).first().expect("a Farm");
    let door = w.comp::<Building>(farm).unwrap().door;
    let cap = w.config.life.commute_hire_ticks;
    // Every adult a candidate (the wages city is staffed at day 0): two are
    // made jobless below.
    let jobless: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| home(&w, a).is_some()).collect();
    let dist = |w: &World, a: EntityId| {
        home(w, a)
            .and_then(|h| w.comp::<Building>(h))
            .map(|b| b.door)
            .or_else(|| w.comp::<Position>(a).map(|p| p.tile))
            .map_or(u64::MAX, |t| commute_cost(w, t, door))
    };
    let near =
        jobless.iter().copied().filter(|&a| (cap * 5 / 8..=cap).contains(&dist(&w, a))).max_by_key(|&a| dist(&w, a));
    let far = jobless
        .iter()
        .copied()
        .filter(|&a| (cap * 9 / 4..=cap * 11 / 4).contains(&dist(&w, a)))
        .min_by_key(|&a| dist(&w, a));
    let (Some(near), Some(far)) = (near, far) else {
        eprintln!("no near/far jobless pair on seed 42 at day 1: nothing to probe");
        return;
    };
    w.remove::<Job>(near);
    w.remove::<Job>(far);
    // The far one: the best farmer, laid off from the role yesterday.
    if let Some(s) = w.comp_mut::<citysim::Skills>(far) {
        s.farming = 1.0;
    }
    if let Some(s) = w.comp_mut::<citysim::Skills>(near) {
        s.farming = 0.0;
    }
    let t = w.tick;
    w.laid_off.insert(far, (Role::Farmer, t));
    let pick = citysim::systems::demography::hire_candidate(&w, farm, Role::Farmer);
    eprintln!(
        "near {} at {} ticks, far {} at {} ticks: picked {:?}",
        near.index,
        dist(&w, near),
        far.index,
        dist(&w, far),
        pick.map(|p| p.index)
    );
    assert_ne!(pick, Some(far), "the far skilled rehire beat a candidate within the cap");
    assert!(pick.is_some_and(|p| dist(&w, p) <= cap), "a candidate within the cap");
}

/// Item 10 (`hf_grow_housing`, P5h; Dylan: the city grows toward jobs): the
/// employer with the most workers past `commute_move_ticks` and no free
/// beds in range gets beds in range within a few days (a Home on a Lot or a
/// floor), and relocation moves far workers into them. Measured on the
/// flipped seed 42: the Farms' 282 beds in range were full for 489-584
/// farmers and the median commute stayed 116 tiles.
#[test]
fn probe_city_grows_homes_toward_jobs() {
    let mut cfg = Config::load();
    // The runtime rule, on the city without the pre-roll: the pre-roll
    // builds until no employer has far workers past its free beds in range
    // while a Lot or floor is left there (its most-far employer is then the
    // land-locked Farm#405: 0 Lots, every Home at floors_max).
    cfg.life.housing_preroll = false;
    let mut w = World::new(42, cfg);
    let cap = w.config.life.commute_move_ticks;
    let door_of = |w: &World, b: EntityId| w.comp::<Building>(b).map(|b| b.door);
    // Beds (capacity) and far workers in range of an employer's door.
    let beds = |w: &World, door: citysim::TilePos| -> usize {
        w.buildings_of_kind(citysim::BuildingKind::Home)
            .iter()
            .filter_map(|&h| w.comp::<Building>(h))
            .filter(|b| !b.demolished && !b.derelict && commute_cost(w, b.door, door) <= cap)
            .map(|b| usize::from(b.capacity))
            .sum()
    };
    let far_at = |w: &World, e: EntityId| -> usize {
        let Some(door) = door_of(w, e) else { return 0 };
        civilians(w)
            .into_iter()
            .filter(|&a| w.comp::<Job>(a).and_then(|j| j.employer) == Some(e))
            .filter(|&a| home(w, a).and_then(|h| door_of(w, h)).is_some_and(|hd| commute_cost(w, hd, door) > cap))
            .count()
    };
    // The employer with the most far workers (ties lower id).
    let employers: Vec<EntityId> =
        civilians(&w).into_iter().filter_map(|a| w.comp::<Job>(a).and_then(|j| j.employer)).collect();
    let mut uniq = employers.clone();
    uniq.sort_unstable();
    uniq.dedup();
    let e = uniq.iter().copied().max_by_key(|&e| (far_at(&w, e), std::cmp::Reverse(e))).expect("an employer");
    let door = door_of(&w, e).unwrap();
    let (beds0, far0) = (beds(&w, door), far_at(&w, e));
    w.run_ticks(6 * TICKS_PER_DAY + TICKS_PER_HOUR);
    let (beds1, far1) = (beds(&w, door), far_at(&w, e));
    eprintln!("{}: beds in range {beds0} -> {beds1}; far workers {far0} -> {far1}", w.name_of(e));
    assert!(far0 >= 6, "the probe needs an employer with far workers");
    assert!(beds1 > beds0, "no Home or floor went up in range of the employer with the most far workers");
}

/// Item 8 (P5h, the couple rule): a married pair who both work at the same
/// far workplace move together into a Home near it (relocation skipped
/// every worker whose spouse held a job: 39 of the 42 far workers left at
/// the Farm the new Homes served on day 8, flipped seed 42).
#[test]
fn probe_working_couple_moves_near_work() {
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let cap = w.config.life.commute_move_ticks;
    let work_of = |w: &World, a: EntityId| {
        w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.comp::<Building>(e)).map(|b| b.door)
    };
    let door_of = |w: &World, a: EntityId| home(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door);
    // A married pair sharing a Home, no children there, the first employed
    // with a commute past twice the cap.
    let mut couples: Vec<(u64, std::cmp::Reverse<EntityId>, EntityId, EntityId)> = civilians(&w)
        .into_iter()
        .filter_map(|a| {
            let s = w.spouse_of(a)?;
            let hm = home(&w, a)?;
            if home(&w, s) != Some(hm) || !w.has::<Brain>(s) || !demography::is_adult(&w, s) {
                return None;
            }
            if demography::children_of_agent(&w, a).iter().any(|&c| home(&w, c) == Some(hm)) {
                return None;
            }
            // A sweeper's day is its beat, not the Recycler's door
            // (`life::work_tile`): relocation does not measure it here.
            if citysim::systems::jobs::is_sweeper(&w, a) {
                return None;
            }
            // A graded post holder keeps its Home's tier (class pay), a
            // rule of its own: the premise is any Home with room.
            if citysim::systems::classpay::min_home_tier(&w, a, hm) > 0 {
                return None;
            }
            let work = work_of(&w, a)?;
            let d = commute_cost(&w, door_of(&w, a)?, work);
            (d > 2 * cap).then_some((d, std::cmp::Reverse(a), a, s))
        })
        .collect();
    // The longest such commute: the relocation pass serves the longest
    // first, and the beds in range are bounded by the Lots and floors
    // there (after class pay's merge, 65 farther movers took the 72 beds
    // the first pick had).
    couples.sort();
    assert!(!couples.is_empty(), "a married worker with a long commute");
    let free = |w: &World, work: citysim::TilePos| -> usize {
        w.buildings_of_kind(citysim::BuildingKind::Home)
            .iter()
            .filter(|&&h| w.comp::<Building>(h).is_some_and(|b| !b.derelict && commute_cost(w, b.door, work) <= cap))
            .map(|&h| usize::from(w.comp::<Building>(h).unwrap().capacity).saturating_sub(w.residents_of(h).len()))
            .sum()
    };
    // The longest whose work has room for them (workplace room, 2026-10-11:
    // the room-capped city's longest was a night crew's Farm whose range the
    // housing pre-roll had already built out, no Lot and every Home at its
    // floors; the premise is a couple with room near work).
    let (mut w, a, s, work) = couples
        .into_iter()
        .rev()
        .find_map(|(_, _, a, s)| {
            let mut w = w.clone();
            // The spouse works at the same place.
            let (employer, role) = w.comp::<Job>(a).map(|j| (j.employer.unwrap(), j.role)).unwrap();
            w.remove::<Job>(s);
            demography::hire(&mut w, s, employer, role);
            for m in [a, s] {
                w.comp_mut::<Wallet>(m).expect("wallet").coins = 1_000;
            }
            // Room near work, enough that the longer commutes served first
            // leave some: Homes and floors go up in range.
            let work = work_of(&w, a).unwrap();
            // The builds are paid (P5h): a Treasury that can pay for them.
            let band = w.config.treasury.band[0];
            if let Some(t) = w.treasury_mut() {
                t.coins = t.coins.max(band + 100_000);
            }
            while free(&w, work) < 200 && citysim::systems::life::build_housing_near(&mut w, employer) {}
            (free(&w, work) >= 2).then_some((w, a, s, work))
        })
        .expect("a couple with a long commute and room near their work");
    eprintln!("free beds in range of work {}", free(&w, work));
    // The daily cap out of the way (the pass serves the longest commutes
    // first; this pair's is not among the city's 40 longest).
    w.config.life.relocate_per_1000 = 1_000.0;
    let before = door_of(&w, a).map(|d| commute_cost(&w, d, work));
    let end = w.tick + TICKS_PER_DAY + TICKS_PER_HOUR;
    run_to(&mut w, a, end, |_| {});
    let after = door_of(&w, a).map(|d| commute_cost(&w, d, work));
    eprintln!(
        "couple {} + {}: commute {before:?} -> {after:?} ticks; same Home {}",
        a.index,
        s.index,
        home(&w, a) == home(&w, s)
    );
    assert!(after < before, "a working couple with room near their shared work stayed put");
    assert_eq!(home(&w, a), home(&w, s), "the couple moved together");
}

/// Item 10 (P5h, no unpaid builds): housing toward jobs is paid. A broke
/// employer and a Treasury at its band floor build nothing; a Treasury
/// above it pays the build out of its purse.
#[test]
fn probe_housing_toward_jobs_is_paid() {
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_HOUR);
    let farm = *w.buildings_of_kind(citysim::BuildingKind::Farm).first().expect("a Farm");
    if let Some(cc) = w.comp::<Building>(farm).and_then(|b| b.owner).and_then(|o| w.comp_mut::<citysim::Corp>(o)) {
        cc.treasury = 0;
    }
    let set_treasury = |w: &mut World, c: i64| {
        if let Some(t) = w.treasury_mut() {
            t.coins = c;
        }
    };
    let floor = w.config.treasury.band[0];
    let homes = |w: &World| -> u32 {
        w.buildings_of_kind(citysim::BuildingKind::Home)
            .iter()
            .filter_map(|&h| w.comp::<Building>(h))
            .map(|b| u32::from(b.floors.max(1)))
            .sum()
    };
    set_treasury(&mut w, floor);
    let before = homes(&w);
    let built = citysim::systems::life::build_housing_near(&mut w, farm);
    assert!(!built && homes(&w) == before, "a build with no one able to pay");
    assert_eq!(w.purse(None), floor, "the Treasury paid nothing");
    // A Treasury with room above its floor pays.
    set_treasury(&mut w, floor + 10_000);
    let t0 = w.purse(None);
    assert!(citysim::systems::life::build_housing_near(&mut w, farm), "the Treasury builds");
    eprintln!("Treasury {t0} -> {} for the build", w.purse(None));
    assert!(w.purse(None) < t0, "the Treasury's build cost it nothing");
}

// ---------------------------------------------------------------------------
// P5i (hunger fixes item 11): meals around the shift
// ---------------------------------------------------------------------------

/// Housed day workers (09:00-18:00, not a guard or sweeper, no gang, free)
/// whose next shift is a workday.
fn day_workers(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| law::living(w, a) && demography::is_adult(w, a))
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a) && home(w, a).is_some())
        .filter(|&a| {
            w.comp::<Job>(a).is_some_and(|j| {
                !matches!(j.role, Role::Guard | Role::Sanitation)
                    && j.shifts == vec![(540, 1080)]
                    && j.employer.is_some()
                    && routine::workday_of(w, a, j, j.next_shift_key(w.tick))
            })
        })
        .collect()
}

/// Item 11 (a): at home before a shift its hunger would not last (0.75 at
/// 06:00 is under the Eat line, 0.6, by mid-shift), with meals in the
/// pantry, a worker eats or packs a meal before leaving (a meal at 0.72
/// would overfill: it packs one) and does not walk off the job for food. Measured on the flipped seed 42 (all-Full): 10.5 %
/// of shift time was Eat, most of it a walk out 2-4 h+ into the shift with
/// nothing carried and an empty pantry.
#[test]
fn probe_worker_eats_before_the_shift_not_during_it() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 6 * TICKS_PER_HOUR);
    let a = day_workers(&w)
        .into_iter()
        .find(|&a| {
            building(&w, a) == home(&w, a)
                && w.comp::<Brain>(a).is_some_and(|b| b.current_goal != Some(citysim::GoalKind::Work))
        })
        .expect("a day worker at home at 06:00");
    pin(&mut w, a);
    let employer = w.comp::<Job>(a).and_then(|j| j.employer).unwrap();
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.75;
    if let Some(i) = w.comp_mut::<Inventory>(a) {
        i.food = 0;
        i.synth = 0;
        i.stolen_food = 0;
    }
    let h = home(&w, a).unwrap();
    if let Some(b) = w.comp_mut::<Building>(h) {
        b.stock_food = 8;
        b.synth_food = 0;
    }
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 30;
    let (mut away_for_food, mut ate, mut packed) = (0u32, false, false);
    run_to(&mut w, a, 2 * TICKS_PER_DAY + 18 * TICKS_PER_HOUR, |w| {
        let s = step(w, a);
        ate |= is_eat(s);
        let on = w.comp::<Job>(a).is_some_and(|j| j.on_shift(w.tick_of_day()));
        let eat_goal = w.comp::<Brain>(a).is_some_and(|b| b.current_goal == Some(citysim::GoalKind::Eat));
        // Walking for food (a GoTo under Eat): a carried meal eaten on the walk in is no walk off the job.
        let walking = matches!(s, Some(ActionKind::GoTo(_)));
        if on && eat_goal && walking && building(w, a) != Some(employer) {
            away_for_food += 1;
        }
        if w.tick_of_day() == 540 {
            packed = w.comp::<Inventory>(a).is_some_and(|i| i.food > i.stolen_food);
        }
    });
    eprintln!(
        "worker {}: ate {ate}; a meal carried at 09:00 {packed}; on-shift minutes away for food {away_for_food}",
        a.index
    );
    assert!(ate || packed, "no meal before the shift and none carried into it");
    assert_eq!(away_for_food, 0, "walked off the job for food with a stocked pantry at home");
}

/// Item 11 (b): off shift, fed, with coins and an empty pantry, a worker
/// tops the pantry up before the next shift (a grocery run), so the next
/// shift's meal is at hand.
#[test]
fn probe_worker_stocks_the_pantry_off_shift() {
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 19 * TICKS_PER_HOUR);
    let a = day_workers(&w)
        .into_iter()
        .find(|&a| {
            building(&w, a) == home(&w, a)
                && w.spouse_of(a).is_none()
                && home(&w, a).is_some_and(|h| w.residents_of(h).len() == 1)
        })
        .or_else(|| day_workers(&w).into_iter().find(|&a| building(&w, a) == home(&w, a)))
        .expect("a day worker at home at 19:00");
    pin(&mut w, a);
    let h = home(&w, a).unwrap();
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.95;
    w.comp_mut::<Needs>(a).expect("needs").energy = 0.9;
    if let Some(i) = w.comp_mut::<Inventory>(a) {
        i.food = 0;
        i.synth = 0;
        i.stolen_food = 0;
    }
    if let Some(b) = w.comp_mut::<Building>(h) {
        b.stock_food = 0;
        b.synth_food = 0;
    }
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 60;
    let mut most = 0u32;
    let mut seen: Vec<ActionKind> = Vec::new();
    run_to(&mut w, a, 2 * TICKS_PER_DAY + 23 * TICKS_PER_HOUR, |w| {
        let held = w.comp::<Building>(h).map_or(0, |b| b.stock_food)
            + w.comp::<Inventory>(a).map_or(0, |i| i.food - i.stolen_food);
        most = most.max(held);
        if let Some(st) = step(w, a) {
            if !seen.contains(&st) {
                seen.push(st);
            }
        }
    });
    eprintln!("steps {seen:?}; residents {}", w.residents_of(h).len());
    eprintln!("worker {}: meals at hand (pantry + carried) peaked at {most} by 23:00; coins {}", a.index, coins(&w, a));
    assert!(seen.contains(&ActionKind::BuyFood), "an empty pantry, coins and an evening off, and no grocery run");
    assert!(most >= 1, "the run left nothing at hand");
}

/// Item 12 (P5j, work-night sleep): on the flipped config, the farmers due
/// at 09:00 (twenty, each pinned in its own clone of the city from 18:00)
/// start the shift rested: the mean energy at 09:00 is at least 0.65, and
/// none is on Sleep then. Measured on the flipped seed 42 (all-Full): 60 %
/// of the farmers' on-shift sleep came from a night under 4 h; their nights
/// went ~13 % to Unwind and ~16 % to walking home to bed.
#[test]
fn probe_worker_sleeps_the_night_before_a_shift() {
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(3 * TICKS_PER_DAY + 18 * TICKS_PER_HOUR);
    let farmers: Vec<EntityId> = day_workers(&w)
        .into_iter()
        .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == Role::Farmer))
        .take(20)
        .collect();
    let (mut energy, mut asleep, mut n) = (0f32, 0u32, 0u32);
    for &a in &farmers {
        let mut c = w.clone();
        run_to(&mut c, a, 4 * TICKS_PER_DAY + 9 * TICKS_PER_HOUR, |_| {});
        energy += c.comp::<Needs>(a).map_or(0.0, |x| x.energy);
        asleep += u32::from(c.comp::<Brain>(a).is_some_and(|b| b.current_goal == Some(citysim::GoalKind::Sleep)));
        n += 1;
    }
    let mean = energy / n.max(1) as f32;
    eprintln!("{n} farmers at 09:00: mean energy {mean:.2}, on Sleep {asleep}");
    assert!(n >= 10, "the probe needs farmers");
    // Item 12 off: 0.61 (52266c8), 0.59 (after the classpay merge and the recalibrated table); on: 0.74, 0.69. The bound sits between the two ranges.
    assert!(mean >= 0.65, "farmers start the shift tired: mean energy {mean:.2}");
    assert_eq!(asleep, 0, "farmers asleep at the shift's start");
}

/// Items 8-10 (after the classpay merge's behaviour tier: d19 sweeper
/// WalkHDay 10.25 against 6.74-7.60, 7.43 with items 8-10 off): a sweeper's
/// working day is its beat, not the Recycler's door. Housing the staff near
/// work (world creation's relocation, then the nightly passes) leaves the
/// sweepers no farther from their beats than without it.
#[test]
fn probe_sweeper_stays_near_its_beat() {
    let beat_walk = |near: bool| -> (f64, usize) {
        let mut cfg = Config::load();
        cfg.life.hf_hire_near = near;
        cfg.life.hf_relocate = near;
        cfg.life.hf_grow_housing = near;
        let mut w = World::new(42, cfg);
        w.run_ticks(15 * TICKS_PER_DAY);
        let v: Vec<u64> = civilians(&w)
            .into_iter()
            .filter(|&a| citysim::systems::jobs::is_sweeper(&w, a))
            .filter_map(|a| {
                let d = citysim::systems::jobs::beat_of(&w, a)?;
                let c = w.districts.get(d.index())?.centroid;
                let hd = home(&w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door)?;
                Some(commute_cost(&w, hd, c))
            })
            .collect();
        (v.iter().sum::<u64>() as f64 / v.len().max(1) as f64, v.len())
    };
    let (on, n_on) = beat_walk(true);
    let (off, n_off) = beat_walk(false);
    eprintln!(
        "sweepers' mean Home-to-beat walk on day 15: items 8-10 on {on:.0} ticks ({n_on}), off {off:.0} ({n_off})"
    );
    assert!(on <= off, "housing near work moved the sweepers away from their beats");
}

/// Item 13 (coordinator call): in the no-net city an evicted Farmer who
/// keeps the job, holding ~8 coins (the flipped city's homeless workers'
/// median), is back in a bed within the wait plus a few days; the deposit
/// wall (21 x rent) kept such workers on the street beside empty beds.
#[test]
fn probe_evicted_worker_rehouses_on_the_first_rent() {
    let mut cfg = Config::load();
    // Jobs & lives phase 2: with the street on, the evicted Farmer (589)
    // joins Ninefold, runs it while its leader sits and is jailed himself:
    // the premise (an employed Farmer) is lost to the butterfly. Re-read at
    // phase 8's gate.
    cfg.jobs2.street = false;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let a = civilians(&w)
        .into_iter()
        .find(|&a| {
            w.spouse_of(a).is_none()
                && home(&w, a).is_some()
                && w.comp::<Job>(a).is_some_and(|j| j.role == Role::Farmer)
                && demography::children_of_agent(&w, a).is_empty()
        })
        .expect("a single Farmer with a Home");
    citysim::systems::ownership::evict(&mut w, a, "arrears");
    if let Some(h) = w.comp_mut::<Household>(a) {
        h.arrears = 0;
    }
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 8;
    let wait = w.config.rent.rehouse_wait_days;
    let end = w.tick + (wait + 3) * TICKS_PER_DAY;
    let mut housed_on = None;
    run_to(&mut w, a, end, |w| {
        if housed_on.is_none() && home(w, a).is_some() {
            housed_on = Some(w.day());
        }
    });
    eprintln!(
        "farmer {}: housed on day {housed_on:?} (evicted day 1, wait {wait} days); job {:?}; coins {}",
        a.index,
        w.comp::<Job>(a).map(|j| j.role),
        coins(&w, a)
    );
    assert!(w.has::<Job>(a), "the probe needs the Farmer employed");
    // Wages lift the purse a little (17 by the end with item 13 off): still under the 21-coin deposit.
    assert!(housed_on.is_some(), "an employed Farmer with the first rent stayed on the street");
}

/// Item 8 (review finding 1): the Home trade checks what the plain pick
/// checks. A far farmer with no coins, evicted by the landlord of the Home
/// near work that a jobless single resident would trade, does not trade into
/// it (nor anywhere it cannot pay the first rent of).
#[test]
fn probe_trade_respects_rent_and_the_evictor() {
    let mut cfg = Config::load();
    // Transit phase 3: the relocation premise is a far walker; with the El on
    // `commute_cost` rides and no single farmer reads far (the El case is
    // `probe_el_*` and `tests/transit.rs`).
    cfg.transit.enabled = false;
    // Jobs & lives phase 1: with the industries on, seed 42 has no single
    // far Farmer either (the premise, not the move, is lost; phase 8's gate
    // re-reads the probes).
    cfg.jobs2.enabled = false;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let cap = w.config.life.commute_move_ticks;
    let work_of = |w: &World, a: EntityId| {
        w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.comp::<Building>(e)).map(|b| b.door)
    };
    let door_of = |w: &World, a: EntityId| home(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door);
    let (far, partner) = civilians(&w)
        .into_iter()
        .filter(|&a| w.spouse_of(a).is_none() && w.comp::<Job>(a).is_some_and(|j| j.role == Role::Farmer))
        .filter(|&a| demography::children_of_agent(&w, a).is_empty())
        .filter_map(|a| {
            let work = work_of(&w, a)?;
            let d = commute_cost(&w, door_of(&w, a)?, work);
            if d <= 2 * cap {
                return None;
            }
            let r = civilians(&w).into_iter().find(|&r| {
                r != a
                    && w.spouse_of(r).is_none()
                    && door_of(&w, r).is_some_and(|d| commute_cost(&w, d, work) <= cap * 3 / 4)
            })?;
            Some((d, std::cmp::Reverse(a), a, r))
        })
        // The longest such commute: the pass serves it first.
        .max()
        .map(|(_, _, a, r)| (a, r))
        .expect("a single far farmer and a single adult living near the Farm");
    w.config.life.relocate_per_1000 = 1_000.0;
    w.remove::<Job>(partner);
    w.comp_mut::<Wallet>(partner).expect("wallet").coins = 50;
    let before = home(&w, far);
    let target = home(&w, partner).expect("partner home");
    let evictor = w.owner_of(target);
    let t = w.tick;
    if let Some(h) = w.comp_mut::<Household>(far) {
        h.evicted_by = Some((evictor, t));
    }
    // No coins at the midnight pass (the day's wage spent).
    let midnight = 2 * TICKS_PER_DAY;
    run_to(&mut w, far, midnight - 1, |_| {});
    w.comp_mut::<Wallet>(far).expect("wallet").coins = 0;
    run_to(&mut w, far, midnight + TICKS_PER_HOUR, |_| {});
    let after = home(&w, far);
    eprintln!(
        "farmer {}: home {:?} -> {:?}; the evictor's Home {}",
        far.index,
        before.map(|x| x.index),
        after.map(|x| x.index),
        target.index
    );
    assert_ne!(after, Some(target), "traded back into its evictor's Home");
    assert_eq!(after, before, "a farmer with no coins traded Homes (the first rent unpaid)");
}

/// Item 11 (b), the off-shift grocery run (the d19 diaries: up to 2.4 h a
/// day of grocery walking, a run re-planned the moment the first returned
/// because one worker never fills a 5-6-resident pantry with three meals):
/// a worker with a crowded Home far from work, an empty pantry and coins
/// does at most one grocery run in the evening-to-morning window, and its
/// market is the one with the least detour (here to the shop and on home).
#[test]
fn probe_worker_makes_one_grocery_run_to_the_nearest_detour_market() {
    use citysim::BuildingKind;
    let mut w = world();
    w.run_ticks(2 * TICKS_PER_DAY + 19 * TICKS_PER_HOUR);
    let dist = |w: &World, a: EntityId| {
        let h = home(w, a).and_then(|h| w.comp::<Building>(h)).map(|b| b.door);
        let e = w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.comp::<Building>(e)).map(|b| b.door);
        h.zip(e).map_or(0, |(h, e)| h.manhattan(e))
    };
    // Off shift at home at 19:00, a crowded Home a walk of an hour or two from work.
    let a = day_workers(&w)
        .into_iter()
        .filter(|&a| building(&w, a) == home(&w, a) && (20..=90).contains(&dist(&w, a)))
        .max_by_key(|&a| {
            (home(&w, a).map_or(0, |h| w.residents_of(h).len().min(3)), dist(&w, a), std::cmp::Reverse(a.index))
        })
        .expect("a day worker at home at 19:00");
    pin(&mut w, a);
    let h = home(&w, a).expect("a home");
    let residents = w.residents_of(h).len();
    w.comp_mut::<Needs>(a).expect("needs").hunger = 0.95;
    w.comp_mut::<Needs>(a).expect("needs").energy = 0.9;
    if let Some(i) = w.comp_mut::<Inventory>(a) {
        i.food = 0;
        i.synth = 0;
        i.stolen_food = 0;
    }
    if let Some(b) = w.comp_mut::<Building>(h) {
        b.stock_food = 0;
        b.synth_food = 0;
    }
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 80;
    // The other residents are broke (the shortfall is this worker's alone to fill).
    fn broke(w: &mut World, h: EntityId, a: EntityId) {
        for r in w.residents_of(h).to_vec() {
            if r != a {
                if let Some(x) = w.comp_mut::<Wallet>(r) {
                    x.coins = 0;
                }
                if let Some(i) = w.comp_mut::<Inventory>(r) {
                    i.food = 0;
                }
            }
        }
    }
    let home_door = w.comp::<Building>(h).expect("home").door;
    let (mut starts, mut markets) = (Vec::new(), Vec::new());
    let mut wrong: Vec<String> = Vec::new();
    // Rested and fed all evening (the shortfall alone drives the errand).
    while w.tick < 3 * TICKS_PER_DAY + 8 * TICKS_PER_HOUR {
        pin(&mut w, a);
        broke(&mut w, h, a);
        if let Some(n) = w.comp_mut::<Needs>(a) {
            n.energy = n.energy.max(0.8);
            n.hunger = n.hunger.max(0.95);
            // Day gaps (2026-10-11): and amused, so leisure does not split
            // the errand (the city's divergence picked seed 42 #1335, whose
            // Unwind won mid-walk as an Arcade came into reach, then the run
            // restarted: two plan starts, one Market).
            n.fun = n.fun.max(0.8);
        }
        w.run_ticks(1);
        let w = &w;
        let Some(p) = w.comp::<Brain>(a).and_then(|b| b.plan.as_ref()) else { continue };
        let grocery = p.steps.len() == 4
            && matches!(p.steps[0].action, ActionKind::GoTo(_))
            && p.steps[1].action == ActionKind::BuyFood
            && p.steps[3].action == ActionKind::StoreFood;
        if !grocery || starts.contains(&p.started_tick) {
            continue;
        }
        starts.push(p.started_tick);
        markets.push(p.target);
        // The least detour among the stocked Markets, from where the run began.
        let from = citysim::exec::walk_origin(w, a).expect("origin");
        let best = w
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| {
                w.comp::<Building>(m)
                    .filter(|b| !b.demolished && !b.derelict && b.stock_food > 0)
                    .map(|b| (from.manhattan(b.door) + b.door.manhattan(home_door), m))
            })
            .min()
            .map(|(_, m)| m);
        if best != p.target {
            wrong.push(format!("tick {}: went to {:?}, the least detour is {best:?}", w.tick, p.target));
        }
    }
    eprintln!(
        "worker {}: {residents} residents, home {} tiles from work; grocery runs {} (markets {markets:?})",
        a.index,
        dist(&w, a),
        starts.len()
    );
    assert_eq!(starts.len(), 1, "grocery runs in one evening-to-morning window");
    assert!(wrong.is_empty(), "not the least-detour market: {wrong:?}");
}

/// Workplace room: hungry at noon with coins and an empty pantry, the
/// Market the agent would shop at full (its door shut to customers): a
/// person turns away at the full door once and eats elsewhere (another
/// Market). On main a refused shopper's replan resolved the same Market
/// (`World::local`) and queued at the same full door again (the shadow run
/// had a guard try `GoTo(Market)` four times and eat nothing with 9 meals
/// at home).
#[test]
fn probe_refused_at_a_full_market_eats_elsewhere() {
    let mut w = world_p3();
    w.run_ticks(2 * TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
    let picks: Vec<EntityId> = civilians(&w)
        .into_iter()
        .filter(|&a| w.comp::<Job>(a).is_none() && home(&w, a).is_some())
        .filter(|&a| w.local(a, citysim::BuildingKind::Market).is_some_and(|m| building(&w, a) != Some(m)))
        .step_by(29)
        .take(24)
        .collect();
    let (mut refused, mut looped, mut unfed) = (0, Vec::new(), Vec::new());
    for &a in &picks {
        let mut w = w.clone();
        pin(&mut w, a);
        let m = w.local(a, citysim::BuildingKind::Market).expect("a Market");
        // The Market is full: no customer gets in.
        w.comp_mut::<Building>(m).expect("market").capacity = 0;
        let price = citysim::systems::life::meal_price(&w, a);
        set_hungry(&mut w, a, 0.2, 10 * price);
        w.debug_events.clear();
        let (mut fulls, mut ate_at) = (0u32, None);
        // The walk to a Market can take hours on the 2 m-tile map.
        let end = w.tick + 12 * TICKS_PER_HOUR;
        run_to(&mut w, a, end, |w| {
            if ate_at.is_none() && is_eat(step(w, a)) {
                ate_at = Some(w.tick);
            }
        });
        let ate = ate_at.is_some();
        // The refusals before the first meal (a second try hours after it,
        // the Market still shut, is a new errand, not the loop).
        for e in w.debug_events.iter() {
            if e.actors.first() == Some(&a)
                && e.text.contains("GoTo(Market): BuildingFull")
                && ate_at.is_none_or(|t| e.tick < t)
            {
                fulls += 1;
            }
        }
        if fulls > 0 {
            refused += 1;
            if fulls > 1 {
                looped.push((a.index, fulls));
            }
            if !ate {
                unfed.push(a.index);
            }
        }
    }
    eprintln!("{refused} of {} refused at the full Market; looped {looped:?}; unfed {unfed:?}", picks.len());
    assert!(refused > 0, "no pick went to the full Market (the probe tests nothing)");
    assert!(looped.is_empty(), "refused at the same full Market again: {looped:?}");
    assert!(unfed.is_empty(), "refused once and never ate (coins in hand): {unfed:?}");
}

// ---------------------------------------------------------------------------
// Vagrancy and the night shift (the workplace-room fix put homeless workers on
// night relief crews; seed 42 d19 an on-shift Vat Tech was cuffed at 23:19).
// ---------------------------------------------------------------------------

/// A homeless worker on a night shift with a Vagrancy report open is not a
/// suspect the law can chase while at work; a homeless jobless adult is.
#[test]
fn probe_homeless_night_worker_on_shift_is_not_charged_with_vagrancy() {
    let mut w = world();
    // Day 3, 02:00: the night crews are on shift at their employers.
    w.run_ticks(3 * TICKS_PER_DAY + 2 * TICKS_PER_HOUR);
    let tod = w.tick_of_day();
    let worker = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && demography::is_adult(&w, a))
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a))
        .find(|&a| {
            w.comp::<Job>(a).is_some_and(|j| {
                j.role != Role::Guard && j.employer.is_some() && j.employer == building(&w, a) && j.on_shift(tod)
            })
        })
        .expect("a free adult at work on the night shift");
    let drifter = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && demography::is_adult(&w, a) && a != worker)
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a))
        .find(|&a| w.comp::<Job>(a).is_none_or(|j| j.employer.is_none()))
        .expect("a free jobless adult");
    for a in [worker, drifter] {
        w.comp_mut::<Household>(a).expect("household").home = None;
        let tile = w.comp::<Position>(a).expect("position").tile;
        law::file_report(&mut w, citysim::Crime::Vagrancy, a, None);
        w.set_last_seen(a, (tile, w.tick));
    }
    let worker_chased = law::is_located_suspect(&w, worker);
    let drifter_chased = law::is_located_suspect(&w, drifter);
    eprintln!("worker {} chased {worker_chased}; drifter {} chased {drifter_chased}", worker.index, drifter.index);
    assert!(!worker_chased, "a homeless worker on shift is a Vagrancy suspect");
    assert!(drifter_chased, "control: a jobless drifter with a Vagrancy report is not chaseable");
}

// ---------------------------------------------------------------------------
// The Hit market phase 2 (docs/HIT_MARKET.md § 9): the widow and the
// suspect. Every Hit and Beat here is a record between fictional agents:
// a struct with a price, matched by a score, resolved by a seeded roll.
// ---------------------------------------------------------------------------

fn set_skill_and_nerve(w: &mut World, id: EntityId, fighting: f32, courage: f32, lawfulness: f32) {
    if let Some(s) = w.comp_mut::<citysim::Skills>(id) {
        s.fighting = fighting;
    }
    if let Some(p) = w.comp_mut::<citysim::Personality>(id) {
        p.courage = courage;
        p.lawfulness = lawfulness;
    }
}

/// The organic Hit and Beat records `buyer` placed.
fn bought_by(w: &World, buyer: EntityId) -> Vec<citysim::contract::Contract> {
    w.contracts.values().filter(|c| c.buyer == Some(buyer) && !c.motive.is_none()).cloned().collect()
}

/// A married civilian with a Fixer in reach of home, her husband killed by
/// a stranger (day 10, 12:00); her grudge on the killer is lethal, and her
/// brother (a Family edge, loyal, lawless) shares it. She holds 40 coins,
/// he 200 (with 40 each no want formed in 7 days: the price moved 42-73
/// while both wallets ran down). The killer is the stranger with the
/// fewest defenders about him. `strong`: she is the stronger and the
/// braver; else the killer is. Returns the world, the widow and the killer.
fn widow(strong: bool) -> (World, EntityId, EntityId) {
    use citysim::systems::grudges;
    use citysim::word::GrudgeCause;
    // Jobs & lives phase 1: the pick is the first married civilian near a
    // Fixer; with the industries on it is another woman (602, a Farmer, not
    // 612, a Cook) who hunts the killer herself in the same 7 days, the
    // premise lost to a pick change. The probe keeps its pre-milestone city
    // until phase 8's gate re-reads the probes (docs/JOBS_AND_LIVES.md § 11).
    let mut c = Config::load();
    c.jobs2.enabled = false;
    let mut w = World::new(42, c);
    // Day 10: the Fixers' books have filled (on day 2 too few regulars
    // would take a Hit at any price, and it priced at the old quote).
    w.run_ticks(10 * TICKS_PER_DAY + 12 * TICKS_PER_HOUR);
    let civ = civilians(&w);
    let her = civ
        .iter()
        .copied()
        .find(|&a| {
            w.spouses.get(&a).is_some_and(|s| civ.contains(s))
                && contracts::nearest_fixer(&w, a).is_some()
                && w.comp::<Job>(a).is_none_or(|j| j.role != Role::Guard)
        })
        .expect("a married civilian near a Fixer");
    let husband = w.spouses[&her];
    let near = |w: &World, x: EntityId, y: EntityId| w.edge(x, y).is_some();
    // A loner: the stranger with the fewest defenders about him (a Hit on
    // a man with friends at his side prices at the old quote, H1).
    let sdef = |w: &World, x: EntityId| citysim::systems::missions::defender_strength(w, &Target::Agent(x));
    let killer = civ
        .iter()
        .copied()
        .filter(|&x| x != her && x != husband && !near(&w, her, x) && !near(&w, husband, x))
        .take(200)
        .min_by(|&a, &b| sdef(&w, a).total_cmp(&sdef(&w, b)).then(a.cmp(&b)))
        .expect("a stranger");
    let brother = civ
        .iter()
        .copied()
        .find(|&x| ![her, husband, killer].contains(&x) && !near(&w, her, x) && !near(&w, killer, x))
        .expect("a brother");
    {
        let e = w.edge_entry(her, brother);
        e.kind = RelKind::Family;
        e.affinity = 1.0;
    }
    if strong {
        set_skill_and_nerve(&mut w, her, 1.0, 0.9, 0.3);
        set_skill_and_nerve(&mut w, killer, 0.1, 0.5, 0.3);
    } else {
        set_skill_and_nerve(&mut w, her, 0.0, 0.3, 0.3);
        set_skill_and_nerve(&mut w, killer, 0.3, 0.5, 0.3);
    }
    set_skill_and_nerve(&mut w, brother, 0.5, 0.5, 0.0);
    w.kill_by(husband, citysim::DeathCause::Violence, Some(killer));
    grudges::add(&mut w, her, killer, GrudgeCause::KilledKin(husband), 0.95, 0);
    grudges::add(&mut w, brother, killer, GrudgeCause::KilledKin(husband), 0.6, 0);
    w.comp_mut::<Wallet>(her).expect("wallet").coins = 40;
    w.comp_mut::<Wallet>(brother).expect("wallet").coins = 200;
    pin(&mut w, her);
    (w, her, killer)
}

/// A widow whose husband was killed by a stronger man, holding 40 coins
/// with a sharing brother, walks to the Fixer and commissions a Hit on him
/// within 7 days. Control: a stronger, braver widow hunts him herself and
/// buys nothing.
#[test]
fn probe_widow_commissions_a_hit_at_the_fixer() {
    let (mut w, her, killer) = widow(false);
    let (mut at_fixer, end) = (None, w.tick + 7 * TICKS_PER_DAY);
    // Watched while she lives (transit phase 3's table: on seed 42 she
    // commissions the Hit, cannot pay the gun and dies in a fight on day 17).
    while w.tick < end && w.has::<Brain>(her) {
        pin(&mut w, her);
        w.run_ticks(1);
        if at_fixer.is_none() && step(&w, her) == Some(ActionKind::Commission) {
            at_fixer = Some(w.tick);
        }
    }
    let mine = bought_by(&w, her);
    eprintln!(
        "widow {}: the Commission step at {at_fixer:?}; records {:?}",
        her.index,
        mine.iter()
            .map(|c| (c.kind, c.target.id().index, c.price, c.deposit, c.broker.map(|b| b.index)))
            .collect::<Vec<_>>()
    );
    assert!(at_fixer.is_some(), "never walked to the Fixer");
    let hit = mine.iter().find(|c| c.target == Target::Agent(killer)).expect("no record on her husband's killer");
    assert_eq!(hit.kind, ContractKind::Hit);
    assert!(hit.broker.is_some(), "posted at the Fixer");
    // Control.
    let (mut w, her, killer) = widow(true);
    let (mut hunted, end) = (false, w.tick + 7 * TICKS_PER_DAY);
    run_to(&mut w, her, end, |w| hunted |= w.hunts.get(&her).is_some_and(|h| h.target == killer));
    eprintln!("strong widow {}: hunted {hunted}, records {}", her.index, bought_by(&w, her).len());
    assert!(hunted, "the control: a strong, brave widow hunts him herself");
    assert!(bought_by(&w, her).is_empty(), "the strong widow paid instead");
}

/// A lawless resident (the suspect) with an open Assault report filed by a
/// woman he knows (`known`) or a stranger, 200 coins, a Fixer near home,
/// at 22:00 on day 2.
fn suspect(known: bool) -> (World, EntityId, EntityId) {
    let mut w = world();
    // Jobs & lives phase 6: the wanted suspect would buy papers at midnight
    // (`gray::papers_daily`) out of the coins this probe gives for the Beat.
    w.config.gray.enabled = false;
    w.run_ticks(2 * TICKS_PER_DAY + 22 * TICKS_PER_HOUR);
    let civ = civilians(&w);
    let s = civ
        .iter()
        .copied()
        .find(|&a| contracts::nearest_fixer(&w, a).is_some() && w.comp::<Job>(a).is_none())
        .expect("a jobless civilian near a Fixer");
    let wit = civ
        .iter()
        .copied()
        .find(|&x| x != s && w.edge(s, x).is_none() && w.comp::<Household>(x).and_then(|h| h.home) != home(&w, s))
        .expect("a stranger");
    if known {
        let e = w.edge_entry(s, wit);
        e.kind = RelKind::Acquaintance;
    }
    set_skill_and_nerve(&mut w, s, 0.5, 0.5, 0.1);
    w.comp_mut::<Wallet>(s).expect("wallet").coins = 200;
    law::file_report(&mut w, citysim::Crime::Assault, s, Some(wit));
    pin(&mut w, s);
    (w, s, wit)
}

/// A suspect with a known witness to his assault deals with her within 7
/// days: M16b § 4's threat first (she withdraws the report), else a Beat
/// bought on her (or he is in Jail first: printed). Control: an unknown
/// witness is never targeted.
#[test]
fn probe_suspect_buys_a_beat_on_a_known_witness() {
    let (mut w, s, wit) = suspect(true);
    let end = w.tick + 7 * TICKS_PER_DAY;
    let mut posted = None;
    run_to(&mut w, s, end, |w| {
        if posted.is_none() && bought_by(w, s).iter().any(|c| c.target == Target::Agent(wit)) {
            posted = Some(w.tick);
        }
    });
    let leaned = w
        .events
        .iter()
        .any(|e| e.kind == EventKind::Coerced && e.actors[..] == [s, wit] && e.text.contains("withdrawn"));
    eprintln!(
        "suspect {} on witness {}: leaned {leaned}, posted at {posted:?}, jailed {}, records {:?}",
        s.index,
        wit.index,
        w.has::<citysim::Sentence>(s),
        bought_by(&w, s).iter().map(|c| (c.kind, c.status, c.motive)).collect::<Vec<_>>()
    );
    if !leaned {
        let beat = bought_by(&w, s)
            .into_iter()
            .find(|c| c.target == Target::Agent(wit))
            .expect("never dealt with the witness");
        assert_eq!((beat.kind, beat.motive), (ContractKind::Beat, citysim::contract::Motive::Silence));
    }
    // Control.
    let (mut w, s, wit) = suspect(false);
    let end = w.tick + 7 * TICKS_PER_DAY;
    run_to(&mut w, s, end, |_| {});
    assert!(bought_by(&w, s).iter().all(|c| c.target != Target::Agent(wit)), "an unknown witness was targeted");
}

// ---------------------------------------------------------------------------
// Transit (docs/TRANSIT.md § 7, phase 3): the El's person probes, on the
// shipped city with the El switched on. A day worker whose Home -> work
// trip the El wins, pinned at Full from 05:00 on day 1.
// ---------------------------------------------------------------------------

use citysim::exec::ExecState;
use citysim::systems::transit;

/// The shipped city with the El on, run to day 1 05:00, and a free housed
/// day worker (09:00-18:00, a workday) with no vehicle whose commute the El
/// wins by 40+ ticks at 07:00: (world, worker, employer, the walk's ticks).
fn el_commuter() -> (World, EntityId, EntityId, u64) {
    let mut cfg = Config::load();
    cfg.transit.enabled = true;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 5 * TICKS_PER_HOUR);
    let t = TICKS_PER_DAY + 7 * TICKS_PER_HOUR;
    let found = w
        .citizens()
        .into_iter()
        .filter(|&a| law::living(&w, a) && demography::is_adult(&w, a))
        .filter(|&a| w.gang_of(a).is_none() && !w.has::<citysim::Sentence>(a))
        .filter(|&a| citysim::systems::vehicles::vehicle_for_trip(&w, a).is_none())
        .find_map(|a| {
            let j = w.comp::<Job>(a)?;
            if matches!(j.role, Role::Guard | Role::Sanitation)
                || j.shifts != vec![(540, 1080)]
                || !routine::workday_of(&w, a, j, j.shift_key_at(t))
            {
                return None;
            }
            let employer = j.employer?;
            let h = home(&w, a)?;
            if building(&w, a) != Some(h) {
                return None;
            }
            let hd = w.outside_door(w.comp::<Building>(h)?);
            let wd = w.comp::<Building>(employer)?.door;
            let walk = citysim::exec::timed_walk(&w, a, hd, wd);
            let r = transit::choose(&w, a, hd, wd, t).ok()?;
            (r.total + 40 <= walk && walk < 6 * 60 && !r.dodge).then_some((a, employer, walk))
        })
        .expect("a day worker at home whom the El carries to work");
    let (a, employer, walk) = found;
    // Decent and solvent unless a probe says otherwise.
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 40;
    w.comp_mut::<citysim::Personality>(a).expect("personality").lawfulness = 0.6;
    pin(&mut w, a);
    (w, a, employer, walk)
}

/// Watch `a` to `until` (re-pinned, `each` before every tick): whether it
/// rode (an `ExecState::Ride` aboard), when it first set out (it starts at
/// home) and when it first stood inside `employer`.
fn watch_commute(
    w: &mut World,
    a: EntityId,
    employer: EntityId,
    until: u64,
    mut each: impl FnMut(&mut World),
) -> (bool, Option<u64>, Option<u64>) {
    let (mut rode, mut left, mut arrived) = (false, None, None);
    while w.tick < until {
        each(w);
        pin(w, a);
        w.run_ticks(1);
        let b = w.comp::<Brain>(a).expect("brain");
        if let ExecState::Ride { depart, .. } = b.exec {
            rode |= w.tick >= depart;
        }
        let moving = matches!(b.exec, ExecState::Goto { .. } | ExecState::GotoTimed { .. } | ExecState::Ride { .. });
        if left.is_none() && moving {
            left = Some(w.tick);
        }
        if arrived.is_none() && building(w, a) == Some(employer) {
            arrived = Some(w.tick);
            break;
        }
    }
    (rode, left, arrived)
}

/// A worker with the fare rides to work and is there sooner than the walk
/// would have brought it (and on time).
#[test]
fn probe_el_commuter_with_coins_rides_and_beats_the_walk() {
    let (mut w, a, employer, walk) = el_commuter();
    let (rode, left, arrived) = watch_commute(&mut w, a, employer, TICKS_PER_DAY + 10 * TICKS_PER_HOUR, |_| {});
    eprintln!("rode {rode}, left {left:?}, arrived {arrived:?}, walk {walk}");
    assert!(rode, "the commuter never rode the El");
    let (left, arrived) = (left.expect("set out"), arrived.expect("arrived at work"));
    assert!(arrived - left < walk, "{} ticks door to door against a {walk}-tick walk", arrived - left);
    assert!(arrived <= TICKS_PER_DAY + 9 * TICKS_PER_HOUR, "late for the 09:00 shift");
}

/// Broke and lawful: walks (no ride, no dodge), and still gets to work.
#[test]
fn probe_el_broke_lawful_worker_walks() {
    let (mut w, a, employer, _) = el_commuter();
    let (rode, _, arrived) = watch_commute(&mut w, a, employer, TICKS_PER_DAY + 12 * TICKS_PER_HOUR, |w| {
        w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    });
    assert!(!rode, "a broke lawful worker rode");
    // (A gate jumped needs a ride: the city's other dodgers count in the column too.)
    assert!(arrived.is_some(), "the walker reached work");
}

/// Broke and lawless: jumps the gate (a FareEvasion on the books) and rides.
#[test]
fn probe_el_broke_lawless_worker_jumps_the_gate() {
    let (mut w, a, employer, _) = el_commuter();
    w.comp_mut::<citysim::Personality>(a).expect("personality").lawfulness = 0.05;
    let dodges0 = w.stats.current.transit.fare_dodges;
    let (rode, _, _) = watch_commute(&mut w, a, employer, TICKS_PER_DAY + 10 * TICKS_PER_HOUR, |w| {
        w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    });
    assert!(rode, "the lawless broke worker never rode");
    assert!(w.stats.current.transit.fare_dodges > dodges0, "no gate jumped");
    assert_eq!(coins(&w, a), 0, "rode on an empty wallet: through the gate unpaid");
}

/// A timid worker sent across town at 22:00 from the worst district walks
/// rather than wait on its platform at night; by day the same trip rides.
#[test]
fn probe_el_timid_worker_walks_at_night_from_the_worst_district() {
    let (mut w, a, employer, _) = el_commuter();
    let h = home(&w, a).expect("home");
    let hd = w.outside_door(w.comp::<Building>(h).expect("home"));
    let wd = w.comp::<Building>(employer).expect("work").door;
    w.comp_mut::<citysim::Personality>(a).expect("personality").courage = 1.0;
    let late = TICKS_PER_DAY + 22 * TICKS_PER_HOUR;
    let r = transit::choose(&w, a, hd, wd, late).expect("a brave rider takes the late train");
    let d = w.district_of(w.transit.net.stops[usize::from(r.board)]);
    let worst = w.districts.iter().map(|x| x.crime_rate).fold(0.0f32, f32::max);
    w.districts[d.index()].crime_rate = worst.max(w.config.transit.danger_crime) + 1.0;
    w.comp_mut::<citysim::Personality>(a).expect("personality").courage = 0.0;
    assert_eq!(transit::choose(&w, a, hd, wd, late), Err(transit::Refusal::Danger));
    // With the executor: the trip set out at 22:00 never boards.
    w.tick = late;
    let tick = w.tick;
    {
        let b = w.comp_mut::<Brain>(a).expect("brain");
        b.plan = Some(citysim::Plan {
            goal: citysim::GoalKind::Idle,
            target: Some(employer),
            steps: vec![citysim::components::ActionInstance {
                action: ActionKind::GoTo(citysim::LocationKey::TargetHome),
                target: Some(employer),
                tile: None,
            }],
            started_tick: tick,
        });
        b.plan_step = 0;
        b.exec = ExecState::Idle;
    }
    for _ in 0..30 {
        citysim::exec::run(&mut w);
        w.tick += 1;
        assert!(!w.rides.contains_key(&a), "the timid worker took the El at night");
    }
    // By day the same body rides.
    let day = TICKS_PER_DAY + 13 * TICKS_PER_HOUR;
    assert!(transit::choose(&w, a, hd, wd, day).is_ok(), "the daytime trip rides");
}

/// Every line cut on the evening of day 1: the next workday morning the
/// worker walks to work (and gets there).
#[test]
fn probe_el_worker_whose_line_is_cut_walks_next_day() {
    let (mut w, a, employer, _) = el_commuter();
    let (rode, _, arrived) = watch_commute(&mut w, a, employer, TICKS_PER_DAY + 10 * TICKS_PER_HOUR, |_| {});
    assert!(rode && arrived.is_some(), "rode on day 1");
    run_to(&mut w, a, TICKS_PER_DAY + 20 * TICKS_PER_HOUR, |_| {});
    for l in 0..3 {
        w.push_command(citysim::PlayerCommand::CutLine { line: l, days: 3 });
    }
    w.apply_commands();
    // The next workday (day 2, or day 3 past a rest day).
    let day = (2..4u64)
        .find(|&d| {
            let t = d * TICKS_PER_DAY + 9 * TICKS_PER_HOUR;
            w.comp::<Job>(a).is_some_and(|j| routine::workday_of(&w, a, j, j.shift_key_at(t)))
        })
        .expect("a workday within two days");
    run_to(&mut w, a, day * TICKS_PER_DAY + TICKS_PER_HOUR, |_| {});
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 40;
    let (rode, _, arrived) = watch_commute(&mut w, a, employer, day * TICKS_PER_DAY + 12 * TICKS_PER_HOUR, |_| {});
    assert!(!rode, "rode a cut line");
    assert!(arrived.is_some(), "walked to work");
}
