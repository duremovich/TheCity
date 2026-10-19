//! Lives at work phase 2 (docs/LIVES_AT_WORK.md § 3, § 9.1): the evening and
//! the household. Unit tests in the shipped seed-42 city with the
//! preconditions set by hand; each failed on the base `70e9589` (no
//! `systems::household`). The behaviour probes are in `tests/person.rs`.
mod common;

use citysim::components::{Brain, Building, BuildingKind, GoalKind, Household, Lod, MemoryEntry, MemoryKind, Position};
use citysim::living::SpotKind;
use citysim::systems::{household, leisure};
use citysim::word::Deed;
use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY, TICKS_PER_HOUR};

/// 19:00 on day 1 (the evening; the day-0 seeding settled).
const EVENING: u64 = TICKS_PER_DAY + 19 * TICKS_PER_HOUR;

fn city() -> World {
    let mut w = common::world();
    assert!(household::on(&w), "the shipped config has the household on");
    w.tick = EVENING;
    w
}

fn home(w: &World, id: EntityId) -> Option<EntityId> {
    w.comp::<Household>(id).and_then(|h| h.home)
}

/// A married, housed, free adult whose spouse shares the Home, both with a Brain.
fn married_pair(w: &World) -> (EntityId, EntityId, EntityId) {
    for a in common::free_adults(w) {
        let Some(s) = w.spouse_of(a) else { continue };
        let Some(h) = home(w, a) else { continue };
        if home(w, s) == Some(h) && w.has::<Brain>(s) && !w.has::<citysim::Sentence>(s) {
            return (a, s, h);
        }
    }
    panic!("a married pair sharing a Home");
}

fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

/// § 3.1: with the spouse inside the Home, the best spot from its door is
/// the Home; with the household off no Home is ever a spot.
#[test]
fn test_best_spot_is_the_home_when_the_spouse_is_in() {
    let mut w = city();
    leisure::spots_daily(&mut w);
    let (a, s, h) = married_pair(&w);
    w.enter_building(s, h);
    w.comp_mut::<Brain>(s).expect("brain").lod = Lod::Full;
    let door = w.comp::<Building>(h).map(|b| w.outside_door(b)).expect("door");
    if let Some(p) = w.comp_mut::<Position>(a) {
        p.tile = door;
        p.building = None;
    }
    let (spot, social) = leisure::best_spot_kind(&w, a, door).expect("a spot");
    assert_eq!(spot.kind, SpotKind::Home(h), "the Home with the spouse in (social {social:.2})");
    assert!(social >= w.config.household.home_w, "the household term counts the spouse ({social:.2})");
    // Off: the corner.
    let mut off = common::world_with(|c| c.household.enabled = false);
    off.tick = EVENING;
    leisure::spots_daily(&mut off);
    off.enter_building(s, h);
    if let Some(p) = off.comp_mut::<Position>(a) {
        p.tile = door;
        p.building = None;
    }
    let (spot, _) = leisure::best_spot_kind(&off, a, door).expect("a spot");
    assert!(!matches!(spot.kind, SpotKind::Home(_)), "no Home spot off");
    assert!(household::home_spots(&off, a, door, 999, true).is_empty());
}

/// § 3.1: the own Home is offered only with company to be had there: an
/// agent living alone (nobody inside, nobody due) has no Home spot, while
/// the married pick with the spouse inside has it.
#[test]
fn test_own_home_is_not_offered_empty() {
    let mut w = city();
    leisure::spots_daily(&mut w);
    let alone = common::free_adults(&w)
        .into_iter()
        .find(|&a| home(&w, a).is_some_and(|h| w.residents_of(h).len() == 1))
        .expect("an adult living alone");
    let h = home(&w, alone).expect("home");
    let door = w.comp::<Building>(h).map(|b| w.outside_door(b)).expect("door");
    assert!(!household::company_due(&w, alone, h), "nobody is in or due at a one-resident Home");
    assert!(household::home_spots(&w, alone, door, 999, false).is_empty(), "no Home spot for a Home with nobody in");
    let (a, s, hh) = married_pair(&w);
    w.enter_building(s, hh);
    let door = w.comp::<Building>(hh).map(|b| w.outside_door(b)).expect("door");
    assert!(household::company_due(&w, a, hh));
    assert_eq!(household::home_spots(&w, a, door, 999, false), vec![(hh, true)]);
}

/// § 3.3: a spot left empty scores × `empty_penalty` for
/// `empty_memory_hours`, and not after; the memory keeps four.
#[test]
fn test_empty_spot_memory_penalises_then_forgets() {
    let mut w = city();
    let a = common::free_adults(&w)[0];
    let t = w.comp::<Position>(a).expect("pos").tile;
    assert_eq!(household::empty_mult(&w, a, t), 1.0);
    household::remember_empty(&mut w, a, t);
    assert_eq!(household::empty_mult(&w, a, t), w.config.household.empty_penalty);
    assert!(household::company_only(&w, a), "after an empty leave, only company");
    assert!(!household::company_at(&w, a, t, None), "nobody at the tile");
    assert_eq!(jobs2(&w).empty_leaves, 1);
    w.tick += u64::from(w.config.household.empty_memory_hours) * TICKS_PER_HOUR;
    assert_eq!(household::empty_mult(&w, a, t), 1.0, "forgotten after empty_memory_hours");
    assert!(!household::company_only(&w, a));
    for i in 0..6u16 {
        household::remember_empty(&mut w, a, citysim::TilePos { x: i, y: 0 });
    }
    assert_eq!(w.comp::<Brain>(a).expect("brain").empty_spots.len(), 4, "the oldest fall off");
}

/// § 3.4: a Birth sets the newborn on both parents, names the carer (the
/// parent without a Job, else the lower wage), damps the carer's night
/// goals and not the other parent's, and lifts both parents' energy decay.
#[test]
fn test_newborn_carer_is_damped_and_the_other_parent_is_not() {
    let mut w = city();
    let (a, s, _) = married_pair(&w);
    // The spouse jobless: the carer.
    w.remove::<citysim::Job>(s);
    assert!(w.has::<citysim::Job>(a) || !w.has::<citysim::Job>(s));
    let child = w.citizens()[0];
    household::on_birth(&mut w, child, a, s);
    assert!(household::new_parent(&w, a) && household::new_parent(&w, s));
    assert!(household::carer(&w, s) && !household::carer(&w, a), "the jobless parent cares");
    assert_eq!(household::energy_mult(&w, a), Some(w.config.household.newborn_energy_mult));
    // The night: 22:00.
    w.tick = TICKS_PER_DAY + 22 * TICKS_PER_HOUR;
    assert!(household::damped(&w, s, GoalKind::Socialise));
    assert!(household::damped(&w, s, GoalKind::Unwind));
    assert!(!household::damped(&w, s, GoalKind::Sleep) && !household::damped(&w, s, GoalKind::Eat));
    assert!(!household::damped(&w, a, GoalKind::Socialise), "the other parent is not damped");
    // Noon: no damping.
    w.tick = TICKS_PER_DAY + 12 * TICKS_PER_HOUR;
    assert!(!household::damped(&w, s, GoalKind::Socialise));
    assert_eq!(household::away_mult(&w, s, false), w.config.household.newborn_away_mult);
    assert_eq!(household::away_mult(&w, s, true), 1.0, "the Home is no away spot");
    assert_eq!(jobs2(&w).carers, 1);
    // At `newborn_until`: over.
    w.tick = w.comp::<Household>(s).and_then(|h| h.newborn_until).expect("until");
    assert!(!household::new_parent(&w, s));
    // The Born deed was talked about.
    let d = citysim::systems::gossip::talk_district(&w, a);
    let posted = w.rumours.get(d.index()).is_some_and(|p| p.entries.iter().any(|e| e.deed == Deed::Born));
    assert!(posted, "the Born deed in the home district's pool");
}

/// § 3.4: `reunite` moves the homeless spouse in when the Home has room,
/// and leaves the pair apart when it has none.
#[test]
fn test_reunite_moves_the_homeless_spouse_in_when_there_is_room() {
    let mut w = city();
    let (a, s, h) = married_pair(&w);
    w.set_home(s, None);
    assert!(home(&w, s).is_none() && home(&w, a) == Some(h));
    household::reunite(&mut w);
    assert_eq!(home(&w, s), Some(h), "reunited");
    assert_eq!(jobs2(&w).reunited, 1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Housed && e.actors[0] == s && e.text.contains("with")));
    // No room: the pair waits.
    w.set_home(s, None);
    let n = w.residents_of(h).len() as u16;
    w.comp_mut::<Building>(h).expect("home").capacity = n;
    household::reunite(&mut w);
    assert!(home(&w, s).is_none(), "no room, no move");
    assert_eq!(jobs2(&w).reunited, 1);
}

/// § 3.4: an heir with a household business waits for it (no outside
/// post) while it takes; after `abandon_days` without a take it is free;
/// the business's open post goes to the household first.
#[test]
fn test_heir_with_a_household_business_keeps_it() {
    let mut w = city();
    let (a, s, _) = married_pair(&w);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    {
        let b = w.comp_mut::<Building>(bar).expect("bar");
        b.owner = Some(a);
        b.revenue_today = 5;
    }
    assert_eq!(household::business_of(&w, a), Some(bar));
    assert_eq!(household::business_of(&w, s), Some(bar), "the spouse's too");
    assert!(household::waits_for_business(&w, s));
    w.remove::<citysim::Job>(s);
    assert_eq!(household::owner_operator(&w, bar, citysim::Role::Bartender), Some(s), "the household first");
    assert!(household::is_member_of_owner(&w, s, bar) && !household::is_member_of_owner(&w, w.citizens()[0], bar));
    // No take for abandon_days: free to take an outside post.
    let days = usize::from(w.config.household.abandon_days);
    {
        let b = w.comp_mut::<Building>(bar).expect("bar");
        b.revenue_today = 0;
        b.revenue.clear();
        for _ in 0..days {
            b.revenue.push_back(0);
        }
    }
    assert!(!household::waits_for_business(&w, s));
}

/// § 3.5: a kin assaulted in the last day (a heard rumour) makes a visit
/// due; the Full visitor's plan walks to the kin and the Rest at its side
/// credits both; a Statistical visitor is credited at the hour.
#[test]
fn test_visit_to_a_hurt_kin_full_and_statistical() {
    let mut w = city();
    // Midnight of day 1 for the pass; the rumour from the evening before.
    w.tick = TICKS_PER_DAY;
    let (a, s, h) = married_pair(&w);
    let heard = MemoryEntry {
        deed: Some(Deed::Assaulted),
        object: Some(s),
        hops: 1,
        ..MemoryEntry::blank(MemoryKind::Rumour, w.tick - TICKS_PER_HOUR)
    };
    w.comp_mut::<citysim::Memory>(a).expect("memory").heard.push(heard);
    household::visits_due(&mut w);
    let v = *w.visits.get(&a).expect("a visit due");
    assert_eq!((v.kin, v.why), (s, household::VisitWhy::Hurt));
    // Before the hour: no plan; at the hour: GoTo the kin, then Rest.
    assert!(household::plan(&mut w, a).is_none());
    w.tick = TICKS_PER_DAY + u64::from(v.hour) * TICKS_PER_HOUR;
    w.enter_building(s, h);
    let p = household::plan(&mut w, a).expect("a plan");
    assert_eq!((p.goal, p.steps.len(), p.target), (GoalKind::Visit, 2, Some(s)));
    assert!(household::considerations(&w, a).is_some());
    // At the kin's side the Rest may start (the plan installed); its end credits the visit.
    w.comp_mut::<Brain>(a).expect("brain").plan = Some(p);
    w.enter_building(a, h);
    assert!(household::can_start(&w, a, citysim::goap::ActionKind::Rest));
    let aff = w.edge(a, s).map(|e| e.affinity).expect("spouses");
    household::visit_done(&mut w, a);
    assert!(!w.visits.contains_key(&a));
    assert_eq!(jobs2(&w).kin_visits, 1);
    assert!(w.edge(a, s).map(|e| e.affinity).expect("edge") > aff, "interacted");
    assert_eq!(w.last_met.get(&(a.min(s), a.max(s))), Some(&w.day()));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Visited && e.actors[0] == a));
    // The Statistical visitor: credited at the hour by `hourly`.
    w.visits.insert(s, household::PendingVisit { kin: a, why: household::VisitWhy::Hurt, hour: v.hour });
    w.comp_mut::<Brain>(s).expect("brain").lod = Lod::Statistical;
    household::hourly(&mut w);
    assert!(!w.visits.contains_key(&s), "credited at its hour");
    assert_eq!(jobs2(&w).kin_visits, 2);
}

/// Shadow V5 bug 3 (§ 1.3, no shift interrupted): a worker on shift with a
/// Visit due keeps working (the Visit's "not on shift" gate is 0; off shift
/// it may win); a Visit whose kin is in no building (nowhere to walk) does
/// not score and is dropped at its plan, not retried at each cooldown; a
/// Visit that cools is dropped for the day. Failed on the base: the Visit
/// scored 1.08 over Work mid-shift and cooled every 2 h, 312 times in V5.
#[test]
fn test_a_visit_waits_out_the_shift_and_an_unplannable_one_is_dropped() {
    let mut w = city();
    let (a, s, h) = married_pair(&w);
    // Noon on day 1 (a workday), the visit's hour past.
    w.tick = TICKS_PER_DAY + 12 * TICKS_PER_HOUR;
    let due = household::PendingVisit { kin: s, why: household::VisitWhy::Hurt, hour: 10 };
    w.visits.insert(a, due);
    w.enter_building(s, h);
    w.remove::<citysim::Job>(a);
    let open = |w: &World| household::considerations(w, a).map(|(cs, _)| cs.iter().all(|c| c.output > 0.0));
    assert_eq!(open(&w), Some(true), "off shift, the kin at home: the Visit may win");
    // On the day shift, at work: the gate holds it.
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    citysim::systems::demography::hire(&mut w, a, market, citysim::Role::Clerk);
    let day = w.config.world.shift_day.clone();
    w.comp_mut::<citysim::Job>(a).expect("hired").shifts = day;
    w.leave_building(a);
    w.enter_building(a, market);
    assert_eq!(open(&w), Some(false), "on shift: the Visit waits for the shift's end");
    assert_ne!(citysim::utility::think(&w, a).map(|(g, _)| g), Some(GoalKind::Visit), "the shift keeps him");
    // Off the job, the kin out on the street: nothing to walk to.
    w.remove::<citysim::Job>(a);
    w.leave_building(s);
    assert!(w.comp::<Position>(s).is_some_and(|p| p.building.is_none()));
    assert!(household::considerations(&w, a).is_none(), "no place to walk: the Visit does not score");
    assert!(household::plan(&mut w, a).is_none());
    assert!(!w.visits.contains_key(&a), "dropped at its plan, not retried");
    // A Visit that cools is dropped for the day.
    w.visits.insert(a, due);
    w.enter_building(s, h);
    assert!(household::plan(&mut w, a).is_some());
    w.cool_goal(a, GoalKind::Visit);
    assert!(!w.visits.contains_key(&a), "a cooled Visit is dropped");
}

/// § 3.5: an elder parent unmet for `elder_visit_days` is visited.
#[test]
fn test_visit_to_an_elder_parent() {
    let mut w = city();
    w.tick = TICKS_PER_DAY;
    let adults = common::free_adults(&w);
    let (child, parent) = (adults[0], adults[1]);
    let frail = w.config.ages.frail_from_years * citysim::time::DAYS_PER_YEAR as u32;
    w.comp_mut::<citysim::Identity>(parent).expect("id").age_days = frail + 10;
    w.comp_mut::<citysim::Identity>(child).expect("id").age_days = frail / 2;
    {
        let e = w.edge_entry(child, parent);
        e.kind = citysim::RelKind::Parent;
        e.affinity = 0.5;
    }
    assert!(citysim::systems::demography::is_child_of(&w, child, parent));
    household::visits_due(&mut w);
    let v = w.visits.get(&child).copied().expect("an elder visit due");
    assert_eq!((v.kin, v.why), (parent, household::VisitWhy::Elder));
    // Met today: none due tomorrow.
    w.last_met.insert((child.min(parent), child.max(parent)), w.day());
    household::visits_due(&mut w);
    assert!(!w.visits.contains_key(&child));
}

/// D13: the off city writes nothing of the phase in three days, and its
/// config holds no `[household]` section nor the phase's deed keys.
#[test]
fn test_off_city_writes_nothing() {
    let mut w = common::world_with(|c| {
        c.household.enabled = false;
        c.without_household_deeds();
    });
    assert!(!household::on(&w));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.visits.is_empty());
    for r in &w.stats.history {
        let j = r.jobs2.as_ref().expect("jobs2 on");
        assert_eq!((j.home_hangouts, j.friend_hangouts, j.home_with_spouse, j.empty_leaves), (0, 0, 0, 0));
        assert_eq!((j.reunited, j.kin_visits, j.elder_visits, j.carers), (0, 0, 0, 0));
    }
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Visited));
    for id in w.citizens() {
        assert!(w.comp::<Brain>(id).is_none_or(|b| b.empty_spots.is_empty()));
        assert!(w.comp::<Household>(id).is_none_or(|h| h.newborn_until.is_none() && !h.newborn_carer));
    }
    assert!(w.config.household.is_default(), "an off household section is the default (not written)");
    let text = ron::to_string(&w.config).expect("config");
    assert!(!text.contains("household:("), "no [household] in an off save's config");
    let deeds = ron::to_string(&w.config.gossip.reach0).expect("table");
    assert!(!deeds.contains("employed") && !deeds.contains("born"), "the phase's deed keys left out: {deeds}");
    // The on config holds them.
    let on = Config::load();
    assert!(on.household.enabled && on.gossip.reach0.born > 0.0);
}

/// The on city lives the phase within two days: HangOuts at home, spots
/// left early, and a visit or a reunion if the city gave one.
#[test]
fn test_on_city_lives_at_home_within_two_days() {
    let mut w = common::world();
    w.run_ticks(2 * TICKS_PER_DAY);
    let j = w.stats.history.iter().fold((0u32, 0u32, 0u32), |a, r| {
        r.jobs2.as_ref().map_or(a, |j| (a.0 + j.home_hangouts, a.1 + j.empty_leaves, a.2 + j.elder_visits))
    });
    assert!(j.0 > 0, "HangOuts at home: {j:?}");
    assert!(j.1 > 0, "empty spots left early: {j:?}");
    assert!(j.2 > 0, "elder visits on the seeded city: {j:?}");
}
