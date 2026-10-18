//! Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1, § 2.1, § 2.2, § 9.1):
//! the service encounter. Unit tests in the shipped seed-42 city with the
//! preconditions set by hand (a Bar's bartenders put on a whole-day shift
//! and inside the Bar; the served queued through `service::arrive`; the
//! hourly pass called directly). Each failed on the base `e5525ee` (no
//! `systems::service`). The probe at the end prints, never asserts.
mod common;

use citysim::components::{Brain, Building, BuildingKind, Job, Lod, Memory, MemoryKind, Position, Role, ServeKind};
use citysim::systems::service;
use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY, TICKS_PER_HOUR};

/// The hour the tests run at: 20:00 on day 0 (every row's evening shift is on).
const EVENING: u64 = 20 * TICKS_PER_HOUR;

/// A whole-day shift whose key is day 0 (a `(0, 1440)` shift keys to the
/// day before and day -1 is a rest day).
const ALL_DAY: (u16, u16) = (1, 1440);

/// The shipped city at 20:00 on day 0 with the encounter on (no tick run:
/// the seeded state).
fn city() -> World {
    let mut w = common::world();
    assert!(service::on(&w), "the shipped config has the encounter on");
    w.tick = EVENING;
    w
}

/// The first building of `kind` with at least `n` staff of `role`, and those staff.
fn staffed(w: &World, kind: BuildingKind, role: Role, n: usize) -> (EntityId, Vec<EntityId>) {
    for &b in w.buildings_of_kind(kind) {
        let staff: Vec<EntityId> = w
            .staff_of(b)
            .iter()
            .copied()
            .filter(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == role) && w.has::<Brain>(s))
            .collect();
        if staff.len() >= n {
            return (b, staff);
        }
    }
    panic!("no {kind:?} with {n} {role:?}");
}

/// Put `s` behind the counter of `b`: a whole-day shift, a Full body inside.
fn on_duty(w: &mut World, s: EntityId, b: EntityId) {
    if let Some(j) = w.comp_mut::<Job>(s) {
        j.shifts = vec![ALL_DAY];
        j.employer = Some(b);
    }
    if let Some(br) = w.comp_mut::<Brain>(s) {
        br.lod = Lod::Full;
    }
    w.enter_building(s, b);
    assert!(service::present(w, s, b), "{} is on duty at {}", w.name_of(s), w.name_of(b));
}

/// Free adults who work nowhere near `b` (the served).
fn guests(w: &World, b: EntityId, n: usize) -> Vec<EntityId> {
    let staff = w.staff_of(b).to_vec();
    common::free_adults(w).into_iter().filter(|a| !staff.contains(a)).take(n).collect()
}

fn served_memories(w: &World, id: EntityId) -> Vec<EntityId> {
    w.comp::<Memory>(id)
        .map(|m| m.entries.iter().filter(|e| e.kind == MemoryKind::Served).filter_map(|e| e.subject).collect())
        .unwrap_or_default()
}

fn served_count(w: &World, kind: ServeKind) -> u32 {
    w.stats.current.jobs2.as_ref().map_or(0, |j| j.served[kind as usize])
}

/// § 1.2: FIFO and capped by `per_hour`: one bartender, 15 drinkers, the
/// first `per_hour` served (their memories name them), the rest waited.
#[test]
fn test_queue_is_fifo_and_capped_by_per_hour() {
    let mut w = city();
    let (bar, staff) = staffed(&w, BuildingKind::Bar, Role::Bartender, 1);
    let s = staff[0];
    on_duty(&mut w, s, bar);
    // Only this one serves (the rest of the Bar's staff off shift).
    for &o in &staff[1..] {
        w.comp_mut::<Job>(o).expect("job").shifts = vec![(0, 1)];
    }
    let per_hour = usize::from(service::row_for(&w, Role::Bartender).expect("row").per_hour);
    let n = per_hour + 3;
    let g = guests(&w, bar, n);
    assert_eq!(g.len(), n);
    for &a in &g {
        service::arrive(&mut w, a, bar, ServeKind::Visit, 2);
    }
    assert_eq!(w.service_room.get(&s).map_or(0, |r| r.taken), per_hour as u8, "the counter is full");
    assert_eq!(served_count(&w, ServeKind::Visit), per_hour as u32);
    // The next hour opens the counter again.
    w.tick += TICKS_PER_HOUR;
    service::hourly(&mut w);
    service::arrive(&mut w, g[0], bar, ServeKind::Visit, 2);
    assert_eq!(served_count(&w, ServeKind::Visit), per_hour as u32 + 1);
    assert_eq!(w.service_room.get(&s).map_or(0, |r| r.taken), 1);
    w.tick -= TICKS_PER_HOUR;
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.served_waited), 3);
    let mem = served_memories(&w, s);
    assert_eq!(mem.len(), per_hour + 1, "the server remembers each one served");
    assert_eq!(mem[..per_hour].to_vec(), g[..per_hour].to_vec(), "FIFO: the first in are served, in order");
    for &a in &g[per_hour..] {
        assert!(served_memories(&w, a).is_empty(), "{} waited", w.name_of(a));
    }
    assert_eq!(service::regulars_of(&w, s).len(), service::REGULARS_CAP.min(per_hour));
}

/// D6: two bartenders on duty, 40 drinkers: both serve some (the hash,
/// never the lowest id), and together they serve `2 x per_hour`.
#[test]
fn test_server_is_a_hash_among_the_present_never_the_lowest_id() {
    let mut w = city();
    let (bar, staff) = staffed(&w, BuildingKind::Bar, Role::Bartender, 2);
    let (a, b) = (staff[0], staff[1]);
    on_duty(&mut w, a, bar);
    on_duty(&mut w, b, bar);
    for &o in &staff[2..] {
        w.comp_mut::<Job>(o).expect("job").shifts = vec![(0, 1)];
    }
    let per_hour = u32::from(service::row_for(&w, Role::Bartender).expect("row").per_hour);
    for &g in &guests(&w, bar, 40) {
        service::arrive(&mut w, g, bar, ServeKind::Visit, 2);
    }
    let (na, nb) = (served_memories(&w, a).len() as u32, served_memories(&w, b).len() as u32);
    assert_eq!(na + nb, 2 * per_hour, "both counters full");
    assert!(na > 0 && nb > 0, "both served some ({na}, {nb}): the pick is a hash, not the lowest id");
    assert_eq!(na, per_hour);
    assert_eq!(nb, per_hour);
}

/// § 1.2's presence: a Statistical bartender on shift is counted present
/// (no body inside), serves, and keeps no `Served` memory; the ledger
/// names the guest all the same.
#[test]
fn test_a_statistical_server_is_counted_present() {
    let mut w = city();
    let (bar, staff) = staffed(&w, BuildingKind::Bar, Role::Bartender, 1);
    let s = staff[0];
    w.comp_mut::<Job>(s).expect("job").shifts = vec![ALL_DAY];
    for &o in &staff[1..] {
        w.comp_mut::<Job>(o).expect("job").shifts = vec![(0, 1)];
    }
    w.comp_mut::<Brain>(s).expect("brain").lod = Lod::Statistical;
    // No learned attendance: the clock credits the whole shift.
    w.stat_table = None;
    assert!(w.comp::<Position>(s).is_some_and(|p| p.building != Some(bar)), "a body elsewhere");
    assert!(service::present(&w, s, bar));
    let g = guests(&w, bar, 1)[0];
    service::arrive(&mut w, g, bar, ServeKind::Visit, 2);
    assert_eq!(served_count(&w, ServeKind::Visit), 1);
    assert!(served_memories(&w, s).is_empty(), "a Statistical agent keeps no Served memory");
    assert_eq!(service::regulars_of(&w, s).iter().map(|r| r.who).collect::<Vec<_>>(), vec![g]);
    // The same bartender held makes nobody present.
    w.comp_mut::<Brain>(s).expect("brain").lod = Lod::Full;
    assert!(!service::present(&w, s, bar), "a body outside the Bar is not behind its counter");
}

/// D5: an encounter writes no memory under a chip (`World::remember`'s
/// rule): a server held under a chip contract keeps nothing of the visit,
/// the guest remembers the server.
#[test]
fn test_no_memory_under_a_chip() {
    use citysim::systems::dollhouse::ChipContract;
    let mut w = city();
    let (bar, staff) = staffed(&w, BuildingKind::Bar, Role::Bartender, 1);
    let s = staff[0];
    on_duty(&mut w, s, bar);
    let house = w.building_of_kind(BuildingKind::Dollhouse).unwrap_or(bar);
    let chip = w.citizens()[0];
    let (since, until) = (w.tick, w.tick + 30 * TICKS_PER_DAY);
    w.jobs2.chip.contracts.insert(s, ChipContract { at: house, holder: None, since, until, chip, captive: true });
    assert!(citysim::systems::dollhouse::under_chip(&w, s));
    let g = guests(&w, bar, 1)[0];
    let before = served_memories(&w, g).len();
    service::serve(&mut w, s, g, bar, ServeKind::Visit, 2);
    assert!(served_memories(&w, s).is_empty(), "nothing is written under the chip");
    assert_eq!(served_memories(&w, g).len(), before + 1, "the guest remembers the server");
    assert_eq!(served_count(&w, ServeKind::Visit), 1);
}

/// § 1.1: the regulars ledger caps at `REGULARS_CAP`; the oldest falls off.
#[test]
fn test_regulars_ledger_caps_at_eight_oldest_off() {
    let mut w = city();
    let (bar, staff) = staffed(&w, BuildingKind::Bar, Role::Bartender, 1);
    let s = staff[0];
    on_duty(&mut w, s, bar);
    let g = guests(&w, bar, service::REGULARS_CAP + 2);
    // Two days apart: the first two are the oldest.
    for (i, &a) in g.iter().enumerate() {
        if i == 2 {
            w.tick += TICKS_PER_DAY;
        }
        service::serve(&mut w, s, a, bar, ServeKind::Visit, 2);
    }
    let kept: Vec<EntityId> = service::regulars_of(&w, s).iter().map(|r| r.who).collect();
    assert_eq!(kept.len(), service::REGULARS_CAP);
    assert_eq!(kept, g[2..].to_vec(), "the two oldest fell off");
}

/// § 1.2 step 3: at `regular_after` encounters within `regular_days` the
/// pair is an Acquaintance, `regulars_made` counts it and a vice row
/// (the brothel worker's; the Lounge has no companion at seed) prints the
/// event; a fourth visit drifts the edge instead.
#[test]
fn test_a_regular_is_made_at_regular_after() {
    let mut w = city();
    let role = w.config.trade_role("brothel_worker").expect("brothel_worker");
    let (house, staff) = staffed(&w, BuildingKind::Brothel, role, 1);
    let s = staff[0];
    on_duty(&mut w, s, house);
    let row = service::row_for(&w, role).expect("row").clone();
    assert_eq!(row.regular_after, 3);
    let g = guests(&w, house, 1)[0];
    assert!(w.edge(s, g).is_none());
    for i in 1..=u32::from(row.regular_after) {
        service::serve(&mut w, s, g, house, ServeKind::Visit, 0);
        assert_eq!(w.edge(s, g).is_some(), i == u32::from(row.regular_after), "an edge at the third only ({i})");
    }
    let made = w.stats.current.jobs2.as_ref().map_or(0, |j| j.regulars_made);
    assert_eq!(made, 1);
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Served && e.text.contains("regular") && e.actors[1] == g),
        "the worker's row prints the regular"
    );
    let aff = w.edge(s, g).map(|e| e.affinity).expect("edge");
    service::serve(&mut w, s, g, house, ServeKind::Visit, 0);
    assert!(w.edge(s, g).map(|e| e.affinity).expect("edge") > aff, "a fourth visit drifts the edge");
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.regulars_made), 1, "made once");
}

/// § 2.1: the tip goes to the server the encounter named (a tipped row),
/// and the madam takes `madam_cut` of it at her Brothel.
#[test]
fn test_tip_to_the_server_and_the_madams_cut() {
    let mut w = city();
    let worker_role = w.config.trade_role("brothel_worker").expect("brothel_worker");
    let madam_role = w.config.trade_role("madam").expect("madam");
    let (house, workers) = staffed(&w, BuildingKind::Brothel, worker_role, 1);
    let worker = workers[0];
    on_duty(&mut w, worker, house);
    let madam = w
        .staff_of(house)
        .iter()
        .copied()
        .find(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == madam_role))
        .expect("a madam at the house");
    on_duty(&mut w, madam, house);
    assert_eq!(service::servers_on_duty(&w, house, ServeKind::Staff), vec![madam]);
    let g = guests(&w, house, 1)[0];
    common::set_coins(&mut w, g, 100);
    let (wk0, md0) = (common::coins(&w, worker), common::coins(&w, madam));
    let price = 20;
    let tip = ((price as f32 * w.config.vice.tip_share).round() as i64).max(1);
    let cut = (tip as f32 * w.config.service.madam_cut).round() as i64;
    assert!(tip > 0 && cut > 0, "tip {tip} cut {cut}");
    service::serve(&mut w, worker, g, house, ServeKind::Visit, price);
    assert_eq!(common::coins(&w, g), 100 - tip, "the guest tipped");
    assert_eq!(common::coins(&w, worker), wk0 + tip - cut, "the worker keeps the tip less the cut");
    assert_eq!(common::coins(&w, madam), md0 + cut, "the madam's cut");
    assert_eq!(w.stats.current.jobs2.as_ref().map_or(0, |j| j.flow_madam_cut), cut);
}

/// § 2.1 (cook): the shift-end rule takes one staff meal from the
/// NoodleBar's stock; off, nothing.
#[test]
fn test_cook_takes_a_staff_meal_at_shift_end() {
    let mut w = city();
    let (bar, cooks) = staffed(&w, BuildingKind::NoodleBar, Role::Cook, 1);
    let cook = cooks[0];
    w.comp_mut::<Building>(bar).expect("bar").stock_food = 5;
    w.comp_mut::<citysim::Needs>(cook).expect("needs").hunger = 0.2;
    service::shift_end(&mut w, cook);
    assert_eq!(w.comp::<Building>(bar).expect("bar").stock_food, 4);
    assert!(w.comp::<citysim::Needs>(cook).expect("needs").hunger > 0.2);
    let j = w.stats.current.jobs2.as_ref().expect("row");
    assert_eq!((j.staff_meals, j.serve_shifts), (1, 1));
    // Off: the shelf untouched.
    let mut off = common::world_with(|c| c.lives.enabled = false);
    off.tick = EVENING;
    off.comp_mut::<Building>(bar).expect("bar").stock_food = 5;
    service::shift_end(&mut off, cook);
    assert_eq!(off.comp::<Building>(bar).expect("bar").stock_food, 5);
}

/// § 2.1 (bouncer): an attacker inside a kept venue is thrown out on
/// `p_eject x (bouncer fighting - attacker fighting)`: with the odds at 1
/// the attacker stands at the door with no plan; a refusal at the door is
/// counted and printed.
#[test]
fn test_bouncer_ejects_and_refuses() {
    let mut w = common::world_with(|c| c.service.p_eject = 10.0);
    w.tick = EVENING;
    let role = w.config.trade_role("bouncer").expect("bouncer");
    let (club, bouncers) = staffed(&w, BuildingKind::Club, role, 1);
    let bouncer = bouncers[0];
    on_duty(&mut w, bouncer, club);
    w.comp_mut::<citysim::Skills>(bouncer).expect("skills").fighting = 1.0;
    let g = guests(&w, club, 1)[0];
    w.comp_mut::<citysim::Skills>(g).expect("skills").fighting = 0.0;
    w.enter_building(g, club);
    assert!(service::maybe_eject(&mut w, g), "thrown out at odds >= 1");
    assert_eq!(w.comp::<Position>(g).and_then(|p| p.building), None, "put at the door");
    assert!(w.comp::<Brain>(g).is_some_and(|b| b.plan.is_none()), "the plan aborted");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Served && e.text.contains("threw")));
    service::refused(&mut w, g, club);
    let j = w.stats.current.jobs2.as_ref().expect("row");
    assert_eq!((j.ejections, j.door_refusals), (1, 1));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Served && e.text.contains("turned")));
    // The attacker's plan would survive a bouncer weaker than him.
    let mut w2 = common::world_with(|c| c.service.p_eject = 10.0);
    w2.tick = EVENING;
    on_duty(&mut w2, bouncer, club);
    w2.comp_mut::<citysim::Skills>(bouncer).expect("skills").fighting = 0.0;
    w2.comp_mut::<citysim::Skills>(g).expect("skills").fighting = 1.0;
    w2.enter_building(g, club);
    assert!(!service::maybe_eject(&mut w2, g), "a stronger attacker is not thrown out");
}

/// § 2.1 (V4 bug 7 on the phase 1 path): a Club's visits are served by its
/// Hosts and dancers by hash, and every tip reaches a tipped dancer on duty:
/// twelve arrivals, two or more dancers gain, no Host does.
#[test]
fn test_club_tips_go_round_the_dancers() {
    let mut w = city();
    let dancer = w.config.trade_role("dancer").expect("dancer");
    let (club, dancers) = staffed(&w, BuildingKind::Club, dancer, 2);
    let hosts: Vec<EntityId> =
        w.staff_of(club).iter().copied().filter(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == Role::Host)).collect();
    assert!(!hosts.is_empty(), "a Club has Hosts");
    for &s in dancers.iter().chain(&hosts) {
        on_duty(&mut w, s, club);
    }
    let before: Vec<i64> = dancers.iter().chain(&hosts).map(|&s| common::coins(&w, s)).collect();
    let g = guests(&w, club, 12);
    for &a in &g {
        common::set_coins(&mut w, a, 100);
        service::arrive(&mut w, a, club, ServeKind::Visit, 10);
    }
    let after: Vec<i64> = dancers.iter().chain(&hosts).map(|&s| common::coins(&w, s)).collect();
    let gained: Vec<bool> = before.iter().zip(&after).map(|(b, a)| a > b).collect();
    let dancers_gained = gained[..dancers.len()].iter().filter(|&&x| x).count();
    assert!(dancers_gained >= 2, "the tips went round the dancers ({dancers_gained} of {})", dancers.len());
    assert!(!gained[dancers.len()..].iter().any(|&x| x), "no Host is tipped");
    let served_by_host = hosts.iter().map(|&h| served_memories(&w, h).len()).sum::<usize>();
    assert!(served_by_host > 0, "the Hosts serve some of the visits");
    let j = w.stats.current.jobs2.as_ref().expect("row");
    assert_eq!(j.tips, 12 * ((10.0f32 * w.config.vice.tip_share).round() as i64).max(1), "every visit tipped");
}

/// D13: the off city writes no `regulars` (the save lacks the key), serves
/// nothing, remembers nothing and prints no `Served` in three days.
#[test]
fn test_off_city_writes_nothing() {
    let mut w = common::world_with(|c| c.lives.enabled = false);
    assert!(!service::on(&w));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.regulars.is_empty() && w.service_room.is_empty());
    // The off save lacks the `regulars` key (other `regulars` maps, a
    // Fixer's stamps, are counted on both sides): the on save of the same
    // day holds exactly one more.
    let off_keys = citysim::save::to_ron(&w).matches("regulars:{").count();
    let mut on = common::world();
    on.run_ticks(TICKS_PER_DAY);
    assert!(!on.regulars.is_empty());
    let on_keys = citysim::save::to_ron(&on).matches("regulars:{").count();
    assert_eq!(on_keys, off_keys + 1, "an off save holds no regulars key");
    for r in &w.stats.history {
        let j = r.jobs2.as_ref().expect("jobs2 on");
        assert_eq!(j.served_total(), 0);
        assert_eq!((j.served_waited, j.serve_shifts, j.staff_meals, j.regulars_made), (0, 0, 0, 0));
        assert_eq!((j.door_refusals, j.ejections, j.flow_madam_cut), (0, 0, 0));
    }
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Served));
    for id in w.citizens() {
        assert!(served_memories(&w, id).is_empty());
    }
    // The env device clears the sections too (the byte identity's half).
    let mut on = Config::load();
    assert!(on.lives.enabled && on.service.venues && !on.serves.is_empty());
    on.lives = Default::default();
    on.service = Default::default();
    on.serves.clear();
    let text = ron::to_string(&on).expect("config");
    assert!(!text.contains("lives:") && !text.contains("service:") && !text.contains("serves:"));
}

/// The on city serves within a day: encounters, regulars' ledgers, Served
/// memories on bodies, and the madam's house among the places.
#[test]
fn test_on_city_serves_within_a_day() {
    let mut w = common::world();
    w.run_ticks(TICKS_PER_DAY);
    let j = w.stats.history.back().and_then(|r| r.jobs2.clone()).expect("jobs2 on");
    assert!(j.served_total() > 0, "encounters on day 0: {:?}", j.served);
    assert!(j.serve_shifts > 0 && j.staff_meals > 0, "shifts {} meals {}", j.serve_shifts, j.staff_meals);
    assert!(!w.regulars.is_empty());
    assert!(w.citizens().into_iter().any(|id| !served_memories(&w, id).is_empty()));
}

/// Print-only: who is behind the counter. Ten days of the shipped city,
/// read at the top of each hour: per venue kind, the share of
/// building-hours with a server on duty for its kind of encounter, the
/// hours of the day with nobody, and why the staff were off (by role: off
/// shift, rest day, a Statistical no-show, a body elsewhere); then the
/// ten days' encounter totals.
#[test]
#[ignore = "tier: diag"]
fn service_probe() {
    use std::collections::BTreeMap;
    let mut w = common::world();
    // kind -> (building-hours, covered), (kind, hour of day) -> uncovered, why -> count
    let mut cover: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut dark: BTreeMap<(String, u16), u32> = BTreeMap::new();
    let mut why: BTreeMap<String, u32> = BTreeMap::new();
    let kinds: Vec<(BuildingKind, ServeKind)> = BuildingKind::LEISURE_ALL
        .iter()
        .map(|&k| (k, ServeKind::Visit))
        .chain([(BuildingKind::Bar, ServeKind::Visit), (BuildingKind::Clinic, ServeKind::Patient)])
        .collect();
    for _ in 0..10 * TICKS_PER_DAY {
        if w.tick.is_multiple_of(TICKS_PER_HOUR) {
            let tod = w.tick_of_day();
            let hour = tod / 60;
            for &(bk, sk) in &kinds {
                for &b in w.buildings_of_kind(bk) {
                    let kind = format!("{bk:?}");
                    let e = cover.entry(kind.clone()).or_insert((0, 0));
                    e.0 += 1;
                    if !service::servers_on_duty(&w, b, sk).is_empty() {
                        e.1 += 1;
                        continue;
                    }
                    *dark.entry((kind.clone(), hour)).or_insert(0) += 1;
                    let staff = w.staff_of(b);
                    if staff.is_empty() {
                        *why.entry(format!("{kind}: no staff at all")).or_insert(0) += 1;
                    }
                    for &s in staff {
                        let Some(j) = w.comp::<Job>(s) else { continue };
                        let row = service::row_for(&w, j.role).map(|r| r.kind);
                        let state = if row != Some(sk) {
                            format!("{:?} serves {row:?}, not {sk:?}", j.role)
                        } else if !j.on_shift(tod) {
                            format!("{:?} off shift", j.role)
                        } else if !citysim::exec::routine::workday_of(&w, s, j, j.shift_key_at(w.tick)) {
                            format!("{:?} rest day", j.role)
                        } else if w.has::<citysim::Sentence>(s) {
                            format!("{:?} held", j.role)
                        } else {
                            match w.comp::<Brain>(s).map(|b| b.lod) {
                                Some(Lod::Statistical) => format!("{:?} Statistical, a no-show", j.role),
                                Some(_) => format!("{:?} a body elsewhere", j.role),
                                None => format!("{:?} no brain", j.role),
                            }
                        };
                        *why.entry(format!("{kind}: {state}")).or_insert(0) += 1;
                    }
                }
            }
        }
        citysim::tick(&mut w);
    }
    for (k, (n, c)) in &cover {
        let hours: Vec<String> =
            (0..24u16).filter_map(|h| dark.get(&(k.clone(), h)).map(|&d| format!("{h}:{d}"))).collect();
        eprintln!(
            "PROBE service {k}: building-hours {n}, a server on duty {c} ({:.2}); uncovered by hour {}",
            f64::from(*c) / f64::from((*n).max(1)),
            hours.join(" ")
        );
    }
    for (k, n) in &why {
        eprintln!("PROBE service why {k}: {n}");
    }
    let j = w.stats.history.iter().fold([0u32; 5], |mut a, r| {
        if let Some(j) = &r.jobs2 {
            a[0] += j.served_total();
            a[1] += j.served_waited;
            a[2] += j.serve_shifts;
            a[3] += j.regulars_made;
            a[4] += j.staff_meals;
        }
        a
    });
    eprintln!(
        "PROBE service 10 days: served {} waited {} staffed shifts {} regulars {} staff meals {}",
        j[0], j[1], j[2], j[3], j[4]
    );
}
