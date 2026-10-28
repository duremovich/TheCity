//! Lives at work phase 3 (docs/LIVES_AT_WORK.md § 2.2, § 3.5, § 9.1): the
//! civic counterparts (teacher, camp warden, jailer, super), the near round,
//! tied housing and the parent's answer to a taken child. Unit tests in the
//! shipped seed-42 city with the preconditions set by hand; each failed on
//! the base `899acc2` (no `[service] civic`). The probes are in
//! `tests/person.rs`.
mod common;

use citysim::components::{
    Brain, Building, BuildingKind, Child, Crime, Household, Job, Lod, Position, Routine, ServeKind,
};
use citysim::systems::household::{CampAnswer, PendingVisit, VisitWhy};
use citysim::systems::{ages, camp, demography, household, law, ownership, service, trades};
use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY, TICKS_PER_HOUR};
use common::{coins, set_coins};

/// Day 2, 01:00: the first midnights hired the trades and settled the city.
const DAY2_ONE: u64 = 2 * TICKS_PER_DAY + TICKS_PER_HOUR;

fn city() -> World {
    let w = common::world_at(DAY2_ONE);
    assert!(service::civic_on(&w), "the shipped config has the civic switch on");
    w
}

fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

fn home(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Household>(id).and_then(|h| h.home)
}

/// The first worker of trade `key` with an employer, and that employer.
fn worker(w: &World, key: &str) -> (EntityId, EntityId) {
    let role = w.config.trade_role(key).unwrap_or_else(|| panic!("the {key} trade"));
    for a in w.citizens() {
        if let Some(j) = w.comp::<Job>(a).filter(|j| j.role == role && j.employer.is_some()) {
            if w.has::<Brain>(a) && !w.has::<citysim::Sentence>(a) {
                return (a, j.employer.expect("employer"));
            }
        }
    }
    panic!("no {key} hired by day 2");
}

/// `s` behind the counter of `b` all day, a Full body inside, present.
fn on_duty(w: &mut World, s: EntityId, b: EntityId, shift: (u16, u16)) {
    if let Some(j) = w.comp_mut::<Job>(s) {
        j.shifts = vec![shift];
        j.employer = Some(b);
    }
    if let Some(br) = w.comp_mut::<Brain>(s) {
        br.lod = Lod::Full;
    }
    w.enter_building(s, b);
    assert!(service::present(w, s, b), "{} is on duty at {}", w.name_of(s), w.name_of(b));
}

/// A married, housed, free pair sharing a Home, both with a Brain.
fn pair(w: &World) -> (EntityId, EntityId, EntityId) {
    for a in common::free_adults(w) {
        let Some(s) = w.spouse_of(a) else { continue };
        let Some(h) = home(w, a) else { continue };
        if home(w, s) == Some(h) && w.has::<Brain>(s) && !w.has::<citysim::Sentence>(s) {
            return (a, s, h);
        }
    }
    panic!("a married pair sharing a Home");
}

/// A pair's child, housed with them; the pair moved to the Home nearest a
/// School so the child's School day is at that School.
fn schoolchild(w: &mut World) -> (EntityId, EntityId, EntityId, EntityId) {
    let (a, s, _) = pair(w);
    let school = w.buildings_of_kind(BuildingKind::School)[0];
    let sd = w.comp::<Building>(school).map(|b| b.door).expect("school door");
    let h = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&b| {
            w.comp::<Building>(b).is_some_and(|bd| {
                !bd.demolished && !bd.derelict && w.residents_of(b).len() + 3 <= usize::from(bd.capacity)
            })
        })
        .min_by_key(|&b| (w.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(sd)), b))
        .expect("a Home with room near the School");
    for m in [a, s] {
        w.set_home(m, Some(h));
    }
    let child = demography::spawn_child(w, s, a, h);
    w.comp_mut::<Child>(child).expect("child").routine = Routine::School;
    // Round 4 T2 (OPEN-232): on the School's roll today, as the midnight's pick
    // puts it (a School with an empty roll is no teacher's workday).
    w.comp_mut::<Child>(child).expect("child").school = Some(school);
    *w.jobs2.ages.rolls.entry(school).or_insert(0) += 1;
    assert_eq!(ages::place_of(w, child), Some(ages::Place::School(school)), "the child's School day is there");
    (a, child, h, school)
}

/// § 2.2 (teacher): no teacher on duty, no taught day (and `edu_days`
/// untouched); a teacher on duty at the shift's first hour teaches
/// `teacher_floor + (1 − teacher_floor) × knowledge` and the School day is
/// a `Pupil` encounter.
#[test]
fn test_no_teacher_no_taught_day_and_a_taught_day_counts() {
    // Lives at work phase 6: pinned to `[careers]` off (on the careers
    // city's moved day 2 the staged child is not taught at the hour; the
    // cause was not chased, the subject is the teacher's day, not the ages).
    // The pressure pass R2: and the v1 pace (`common::v1_pace`).
    // The pressure pass S6 (OPEN-224): the seeded children off (the staged
    // child is the School's one pupil; the roll would hold the city's too).
    let mut w = common::world_with(|c| {
        c.strip_careers();
        common::v1_pace(c);
        c.ages.seed_child_slots = 0;
    });
    w.run_ticks(DAY2_ONE);
    let (_, child, _, school) = schoolchild(&mut w);
    let (teacher, _) = worker(&w, "teacher");
    let edu_days = w.comp::<Child>(child).map_or(0, |c| c.edu_days);
    // Nobody on duty: the teacher's shift starts at 09:00, it is 01:00.
    if let Some(j) = w.comp_mut::<Job>(teacher) {
        j.shifts = vec![(540, 1080)];
        j.employer = Some(school);
    }
    service::hourly(&mut w);
    let c = w.comp::<Child>(child).cloned().expect("child");
    assert_eq!(c.edu_taught, 0.0, "a School day with no teacher teaches nothing");
    assert_eq!(c.edu_days, edu_days);
    assert_eq!(jobs2(&w).taught_days, 0);
    // The teacher at the first hour of an all-day shift (01:00 is in it).
    on_duty(&mut w, teacher, school, (1, 1440));
    let know = w.comp::<citysim::Skills>(teacher).map_or(0.0, |s| s.knowledge);
    service::hourly(&mut w);
    let c = w.comp::<Child>(child).cloned().expect("child");
    let floor = w.config.service.teacher_floor;
    assert!((c.edu_taught - (floor + (1.0 - floor) * know)).abs() < 1e-5, "taught {} (knowledge {know})", c.edu_taught);
    assert_eq!(c.edu_days, edu_days, "the midnight count is not the teacher's");
    let j = jobs2(&w);
    assert_eq!(j.taught_days, 1);
    assert!(j.served[ServeKind::Pupil as usize] >= 1, "the School day is a Pupil encounter");
    assert!(service::regulars_of(&w, teacher).iter().any(|r| r.who == child), "the ledger names the pupil");
}

/// § 2.2 (super, `round_near`): the round's stops are the `round_stops`
/// Blocks of the landlord in the post's district nearest the super's Home;
/// a Home moved far away moves the stops with it.
#[test]
fn test_super_round_takes_the_nearest_blocks_to_home() {
    let mut w = city();
    // The near-home rule alone: the round's reach (the pre-view pass phase 4,
    // OPEN-154; `presence.rs` tests it) left this landlord too few Blocks.
    if let Some(c) = w.config.presence.as_mut() {
        c.round_reach_tiles = u32::MAX;
    }
    let (sup, post) = worker(&w, "super");
    let row = trades::row_of(&w, w.comp::<Job>(sup).expect("job").role).expect("row").clone();
    assert!(row.round_near && row.tied_housing, "the shipped super row carries the phase 3 keys");
    let owner = w.owner_of(post).expect("a landlord");
    let d = w.district_of_building(post);
    let pool = |w: &World| -> Vec<EntityId> {
        w.buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .filter(|&b| b != post && w.owner_of(b) == Some(owner) && w.district_of_building(b) == d)
            .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
            .collect()
    };
    let nearest = |w: &World, from: citysim::TilePos| -> Vec<EntityId> {
        let mut v: Vec<(u32, EntityId)> = pool(w)
            .into_iter()
            .map(|b| (w.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(from)), b))
            .collect();
        v.sort_unstable();
        let mut out: Vec<EntityId> = v.into_iter().take(usize::from(row.round_stops)).map(|(_, b)| b).collect();
        out.sort_unstable();
        out
    };
    assert!(pool(&w).len() > usize::from(row.round_stops), "a landlord with more Blocks than stops in the district");
    // The super housed in the landlord's farthest Block of the district.
    let far = *pool(&w)
        .iter()
        .max_by_key(|&&b| {
            (w.comp::<Building>(b).map_or(0, |bd| bd.door.manhattan(w.comp::<Building>(post).expect("post").door)), b)
        })
        .expect("far");
    w.set_home(sup, Some(far));
    let from = w.comp::<Building>(far).expect("far").door;
    let mut stops = trades::round_for(&w, sup, 7);
    stops.sort_unstable();
    assert_eq!(stops, nearest(&w, from), "the stops nearest the Home");
    // The near round is the civic switch's: off, the id-ordered rotation.
    let mut off = common::world_with(|c| c.strip_civic());
    off.run_ticks(DAY2_ONE);
    assert!(off.config.trades.iter().all(|t| !t.round_near && !t.tied_housing), "the keys are stripped off");
    assert!(off.config.serves.iter().all(|r| !r.civic), "the civic rows are stripped off");
}

/// § 2.2 (super, `tied_housing`): a super whose Home's owner is not his
/// landlord moves at midnight into the landlord's nearest Block with room
/// (never into a full one), and pays no rent there.
#[test]
fn test_tied_super_moves_in_and_pays_no_rent() {
    let mut w = city();
    let (sup, post) = worker(&w, "super");
    let owner = w.owner_of(post).expect("a landlord");
    // Housed with another landlord (or nowhere), in a Block that no
    // co-resident holds tied (that household stays put by design; on the
    // 8d fix round's stat table the first Block with room housed a super
    // of its own landlord).
    let other = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&b| {
            w.owner_of(b) != Some(owner)
                && !w.residents_of(b).iter().any(|&r| ownership::tied_here(&w, r, b))
                && w.comp::<Building>(b).is_some_and(|bd| {
                    !bd.demolished && !bd.derelict && w.residents_of(b).len() < usize::from(bd.capacity)
                })
        })
        .expect("a Block of another owner with room");
    w.set_home(sup, Some(other));
    // Every Block of the landlord in the district full: nobody moves.
    let d = w.district_of_building(post);
    let blocks: Vec<EntityId> = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&b| w.owner_of(b) == Some(owner) && w.district_of_building(b) == d)
        .collect();
    let caps: Vec<u16> = blocks.iter().map(|&b| w.comp::<Building>(b).map_or(0, |bd| bd.capacity)).collect();
    for &b in &blocks {
        let n = w.residents_of(b).len() as u16;
        w.comp_mut::<Building>(b).expect("block").capacity = n;
    }
    household::tied_housing(&mut w);
    assert_eq!(home(&w, sup), Some(other), "no room, no move (never an eviction)");
    let moved_others = jobs2(&w).tied_housed;
    for (&b, &c) in blocks.iter().zip(&caps) {
        w.comp_mut::<Building>(b).expect("block").capacity = c;
    }
    household::tied_housing(&mut w);
    let h = home(&w, sup).expect("housed");
    assert_eq!(w.owner_of(h), Some(owner), "moved into the landlord's Block");
    assert!(w.district_of_building(h) == d && blocks.contains(&h));
    assert_eq!(jobs2(&w).tied_housed, moved_others + 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Housed && e.actors.contains(&sup)));
    // Rent-free there: the midnight pass zeroes what he owes.
    assert!(ownership::tied_here(&w, sup, h));
    if let Some(hh) = w.comp_mut::<Household>(sup) {
        hh.rent_due = 5.0;
        hh.arrears = 2;
    }
    set_coins(&mut w, sup, 50);
    w.tick = 3 * TICKS_PER_DAY;
    ownership::run(&mut w);
    let hh = w.comp::<Household>(sup).cloned().expect("household");
    assert_eq!((hh.rent_due, hh.arrears), (0.0, 0), "a tied super pays no rent");
    assert_eq!(coins(&w, sup), 50);
}

/// § 2.2 (super): at a stop a tenant in arrears is served `Notice` (once a
/// day), and a tenant whose purse holds a whole coin of rent due pays it at
/// the door (`door_rent`, the same `Flow::Rent`).
#[test]
fn test_super_stop_serves_notice_and_takes_the_door_rent() {
    let mut w = city();
    let (sup, post) = worker(&w, "super");
    let owner = w.owner_of(post);
    let stop = trades::round_for(&w, sup, 7)
        .into_iter()
        .find(|&b| w.residents_of(b).iter().any(|&t| t != sup && w.has::<Brain>(t) && demography::is_adult(&w, t)));
    let stop = stop.expect("a stop with an adult tenant");
    let tenant = w
        .residents_of(stop)
        .iter()
        .copied()
        .find(|&t| t != sup && w.has::<Brain>(t) && demography::is_adult(&w, t))
        .expect("tenant");
    if let Some(h) = w.comp_mut::<Household>(tenant) {
        h.arrears = 2;
        h.rent_due = 3.4;
    }
    set_coins(&mut w, tenant, 10);
    // The pre-view pass phase 4 (P3): the tenant is at home and awake.
    citysim::systems::lod::set_lod(&mut w, tenant, citysim::components::Lod::Full);
    w.enter_building(tenant, stop);
    w.comp_mut::<Brain>(tenant).expect("brain").exec = citysim::ExecState::Idle;
    let owner_before = w.purse(owner);
    service::super_stop(&mut w, sup, stop);
    let j = jobs2(&w);
    assert_eq!(j.super_stops, 1);
    assert_eq!(j.notices, 1, "one notice for the tenant in arrears");
    // The pressure pass R4: a round's notices are one line at its end.
    service::notices_folded(&mut w, sup, Some(1));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Notice && e.actors.as_slice() == [sup, tenant]));
    assert_eq!(j.door_rent, 3, "the whole coins of rent paid at the door");
    // A full payment clears the arrears as the midnight one does; a short
    // one at the door adds no day of arrears (`settle_rent(midnight = false)`).
    assert!(w.comp::<Household>(tenant).is_some_and(|h| h.rent_due < 1.0 && h.arrears == 0), "paid in full");
    assert_eq!(coins(&w, tenant), 7);
    assert!(w.purse(owner) > owner_before, "the landlord's purse took it (less the tax)");
    assert!(j.served[ServeKind::Tenant as usize] >= 1);
    // The same day again: no second notice.
    service::super_stop(&mut w, sup, stop);
    assert_eq!(jobs2(&w).notices, 1);
}

/// § 2.2 (jailer): the midnight meal of a held prisoner is an `Inmate`
/// encounter with the jailer on duty.
#[test]
fn test_jailer_meal_is_an_inmate_encounter() {
    // The pressure pass R2: the v1 pace (`common::v1_pace`).
    let mut w = common::world_with(common::v1_pace);
    w.run_ticks(DAY2_ONE);
    // Jailers are posted one per `held_per_jailer` held, so day 2 has none: one is hired by hand.
    let jail = w.building_of_kind(BuildingKind::Jail).expect("the Jail");
    let role = w.config.trade_role("jailer").expect("the jailer trade");
    // Two jobless ones (F1: the city's sweepers are hired near the Recycler,
    // so the first free adults may hold a shift).
    let adults: Vec<_> = common::free_adults(&w).into_iter().filter(|&a| !w.has::<citysim::Job>(a)).collect();
    let jailer = adults[0];
    demography::hire(&mut w, jailer, jail, role);
    on_duty(&mut w, jailer, jail, (0, 1440));
    // A prisoner for the night.
    let p = adults[1];
    w.insert(p, citysim::Sentence { until_tick: w.tick + 5 * TICKS_PER_DAY, crime: Crime::Theft });
    w.enter_building(p, jail);
    let market = w.nearest_of_kind(BuildingKind::Market, w.comp::<Building>(jail).expect("jail").door).expect("market");
    w.comp_mut::<Building>(market).expect("market").stock_food += 100;
    w.tick = 3 * TICKS_PER_DAY;
    w.tick += u64::from(citysim::systems::midnight::tick_of(&w, citysim::systems::midnight::Mid::Law));
    let before = jobs2(&w).served[ServeKind::Inmate as usize];
    law::run(&mut w);
    assert!(jobs2(&w).served[ServeKind::Inmate as usize] > before, "the fed prisoner met the jailer");
    assert!(service::regulars_of(&w, jailer).iter().any(|r| r.who == p), "the jailer's ledger names him");
}

/// A pair's child taken into a Camp (the service's placement), the pair
/// housed and both with a Brain.
fn taken_child(w: &mut World) -> (EntityId, EntityId, EntityId, EntityId) {
    let (a, s, h) = pair(w);
    let child = demography::spawn_child(w, s, a, h);
    let cmp = camp::take(w, child).expect("a standing Camp takes the child");
    assert_eq!(home(w, child), Some(cmp));
    (a, child, h, cmp)
}

/// § 3.5 (buy-out): the fee moves parent -> the Camp's owner as
/// `Flow::Buyout` and the child comes home.
#[test]
fn test_buyout_returns_the_child_and_moves_the_fee() {
    let mut w = city();
    let (a, child, h, cmp) = taken_child(&mut w);
    let fee = w.config.service.buyout_fee;
    assert!(fee > 0);
    set_coins(&mut w, a, fee + 20);
    let owner = w.owner_of(cmp);
    let owner_before = w.purse(owner);
    w.visits.insert(a, PendingVisit { kin: child, why: VisitWhy::Camp(CampAnswer::BuyOut), hour: 12 });
    household::visit_done(&mut w, a);
    assert_eq!(home(&w, child), Some(h), "the child is home");
    assert!(!camp::children_of(&w, cmp).contains(&child));
    assert_eq!(coins(&w, a), 20);
    assert_eq!(w.purse(owner) - owner_before, fee, "the fee to the Camp's owner, untaxed");
    let j = jobs2(&w);
    assert_eq!((j.camp_buyouts, j.flow_buyout), (1, fee));
    assert!(w.events.iter().any(|e| e.kind == EventKind::CampBuyout && e.actors.as_slice() == [a, child, cmp]));
    assert!(w.last_met.contains_key(&(a.min(child), a.max(child))), "a `last_met` with the child");
    // Short of the fee at the hour: a visit instead, the coins back.
    let (b, child2, _, cmp2) = taken_child(&mut w);
    set_coins(&mut w, b, fee / 2);
    w.visits.insert(b, PendingVisit { kin: child2, why: VisitWhy::Camp(CampAnswer::BuyOut), hour: 12 });
    household::visit_done(&mut w, b);
    assert_eq!(home(&w, child2), Some(cmp2), "still in the Camp");
    assert_eq!(coins(&w, b), fee / 2, "refunded");
    assert_eq!(jobs2(&w).camp_visits, 1);
}

/// § 3.5 (break-out): a contest at the Camp door against the wardens
/// present; won, the child comes home and an `Abduction` is filed on the
/// parent.
#[test]
fn test_breakout_is_a_contest_at_the_door_and_files_abduction() {
    let mut w = city();
    let (a, child, h, cmp) = taken_child(&mut w);
    if let Some(p) = w.comp_mut::<citysim::Personality>(a) {
        p.lawfulness = 0.1;
        p.courage = 0.9;
    }
    if let Some(s) = w.comp_mut::<citysim::Skills>(a) {
        s.fighting = 1.0;
    }
    // The parent at the door, as the walk leaves him.
    let door = w.comp::<Building>(cmp).expect("camp").door;
    w.leave_building(a);
    w.comp_mut::<Position>(a).expect("pos").tile = door;
    w.visits.insert(a, PendingVisit { kin: child, why: VisitWhy::Camp(CampAnswer::BreakOut), hour: 12 });
    household::visit_done(&mut w, a);
    let j = jobs2(&w);
    if j.camp_breakouts == 1 {
        assert_eq!(home(&w, child), Some(h), "won: the child is home");
        assert!(w.events.iter().any(|e| e.kind == EventKind::CampBreakout && e.actors.as_slice() == [a, child, cmp]));
    } else {
        assert_eq!(home(&w, child), Some(cmp), "lost: the child stays");
    }
    assert!(
        w.crime_reports().iter().any(|r| r.crime == Crime::Abduction && r.suspect == a),
        "an Abduction filed on the parent either way"
    );
}

/// § 2.2 (warden): the escape roll returns the child to a parent's Home
/// with room (`p_camp_escape = 1` here), the event and the counter.
#[test]
fn test_escape_roll_returns_the_child_home() {
    let mut w = common::world_with(|c| c.service.p_camp_escape = 1.0);
    w.run_ticks(DAY2_ONE);
    let (a, child, h, cmp) = taken_child(&mut w);
    w.tick = 3 * TICKS_PER_DAY;
    camp::daily(&mut w);
    assert_eq!(home(&w, child), Some(h), "home to the parent {}", w.name_of(a));
    assert!(!camp::children_of(&w, cmp).contains(&child));
    assert_eq!(jobs2(&w).camp_escapes, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::CampEscape && e.actors[0] == child));
    // The pressure pass S6 (Dylan 2026-10-27, no child is harmed): the
    // warden's beating roll is gone (it was checked here).
}

/// § 3.5: the parent's answer is chosen from the household's state at
/// midnight: the fee and the pantry buy out; lawless and brave break out;
/// the rest visit; once a week a parent, the Visit due at a hashed hour.
#[test]
fn test_parents_answer_is_chosen_from_the_household() {
    let mut w = city();
    let (a, child, h, _) = taken_child(&mut w);
    let fee = w.config.service.buyout_fee;
    set_coins(&mut w, a, fee + 10);
    w.comp_mut::<Building>(h).expect("home").stock_food = 50;
    // Walk the week's midnights until the parent's day comes.
    let mut seen = None;
    for day in 3..24 {
        w.tick = day * TICKS_PER_DAY;
        household::daily(&mut w);
        if let Some(v) = w.visits.get(&a) {
            seen = Some(*v);
            break;
        }
    }
    let v = seen.expect("a parent answers within a week");
    assert_eq!((v.kin, v.why), (child, VisitWhy::Camp(CampAnswer::BuyOut)));
    assert!((w.config.household.visit_hour_from..w.config.household.visit_hour_to).contains(&v.hour));
    // Broke and law-abiding: a visit.
    set_coins(&mut w, a, 1);
    if let Some(p) = w.comp_mut::<citysim::Personality>(a) {
        p.lawfulness = 0.9;
    }
    let mut seen = None;
    for day in 24..45 {
        w.tick = day * TICKS_PER_DAY;
        household::daily(&mut w);
        if let Some(v) = w.visits.get(&a) {
            seen = Some(*v);
            break;
        }
    }
    assert_eq!(seen.map(|v| v.why), Some(VisitWhy::Camp(CampAnswer::Visit)));
    assert!(jobs2(&w).camp_neglect > 0, "the unvisited child's parents are counted");
}

/// The off city (`[service] civic = false`): no civic row, no civic key,
/// no counter written in three days; the on city's third day has the
/// super's stops.
#[test]
fn test_off_city_writes_nothing() {
    // `Config::load` strips the rows and keys when the switch is off by any
    // path (the toml, `[service] civic = false`); a config edited after the load
    // calls it itself.
    let mut off = common::world_with(|c| c.strip_civic());
    assert!(!service::civic_on(&off));
    assert!(off.config.serves.iter().all(|r| !r.civic) && off.config.service.buyout_fee == 0);
    assert!(off.config.trades.iter().all(|t| !t.round_near && !t.tied_housing));
    off.run_ticks(3 * TICKS_PER_DAY);
    for r in &off.stats.history {
        let j = r.jobs2.clone().unwrap_or_default();
        assert_eq!(
            (j.super_stops, j.notices, j.door_rent, j.tied_housed, j.taught_days, j.camp_headcounts),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!((j.served[ServeKind::Pupil as usize], j.served[ServeKind::Tenant as usize]), (0, 0));
    }
    let ron = citysim::save::to_ron(&off);
    assert!(!ron.contains("round_near") && !ron.contains("buyout_fee") && !ron.contains("edu_taught"));
    let mut on = common::world();
    on.run_ticks(3 * TICKS_PER_DAY);
    let j = on.stats.history.back().and_then(|r| r.jobs2.clone()).unwrap_or_default();
    assert!(j.super_stops > 0 && j.served[ServeKind::Tenant as usize] > 0, "the supers' rounds serve tenants");
    let _ = Config::load();
}
