//! Jobs & lives phase 7, life stages (docs/JOBS_AND_LIVES.md § 6 and § 12's
//! probes): a vendor's child is at the pitch on a weekday, the same child
//! with a School in reach and the fee paid is at the School; a fired elder
//! moves in with a grown daughter within a week (the control, with none,
//! stays put); a stabilised survivor maimed works slower and a prosthetic
//! clears it; the seeded ages run to 75 under the switch. Each test is the
//! shipped seed-42 city and one mechanism, driven directly.

mod common;

use citysim::components::{Pitch, RelKind};
use citysim::systems::ages;
use citysim::{Brain, BuildingKind, Config, EntityId, Household, Identity, Job, Wallet, World, TICKS_PER_DAY};

fn on_world() -> World {
    World::new(42, Config::load())
}

/// Free jobless adults outside any gang (ascending).
fn idle(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(w, a)
                && w.gang_of(a).is_none()
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
        })
        .collect()
}

fn coins(w: &mut World, a: EntityId, n: i64) {
    if let Some(x) = w.comp_mut::<Wallet>(a) {
        x.coins = n;
    }
}

/// Walk the clock to the next weekday midnight.
fn to_weekday(w: &mut World) {
    while w.day() % 7 >= 5 {
        w.tick += TICKS_PER_DAY;
    }
}

/// § 12's probe: a vendor's child is at the pitch on a weekday (Chores);
/// the same child with a School in reach and the fee paid is at the School.
#[test]
fn test_vendors_child_chores_then_school() {
    let mut w = on_world();
    let school = *w.buildings_of_kind(BuildingKind::School).first().expect("a School is seeded");
    let door = w.comp::<citysim::Building>(school).map(|b| b.door).expect("door");
    let home = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .filter(|&h| w.comp::<citysim::Building>(h).is_some_and(|b| !b.derelict && b.door.manhattan(door) <= 30))
        .min()
        .expect("a Home near the School");
    let pool = idle(&w);
    let (mother, father) = (pool[0], pool[1]);
    for p in [mother, father] {
        w.set_home(p, Some(home));
        coins(&mut w, p, 0);
    }
    let kid = citysim::systems::demography::spawn_child(&mut w, mother, father, home);
    let trade = w.config.trade_role("vendor").and_then(|r| r.trade()).expect("the vendor row");
    let at = w.comp::<citysim::Position>(mother).expect("pos").tile;
    let p = Pitch {
        spot: at,
        district: w.district_of(at),
        trade,
        owner: mother,
        since: w.tick,
        take_today: 0,
        take_week: 0,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
        cut_paid: 0,
    };
    w.jobs2.pitches.insert(mother, p);
    to_weekday(&mut w);
    ages::daily(&mut w);
    assert_eq!(ages::place_of(&w, kid), Some(ages::Place::Pitch(at)), "at the pitch: no fee to spare");
    coins(&mut w, mother, 500);
    coins(&mut w, father, 500);
    ages::daily(&mut w);
    assert!(matches!(ages::place_of(&w, kid), Some(ages::Place::School(_))), "at the School, the fee paid");
}

/// § 12's probe: a 64-year-old Farmer let go moves in with his grown
/// daughter within 7 days; the control, with no daughter, stays put.
#[test]
fn test_fired_elder_moves_in_with_his_daughter() {
    let run = |daughter: bool| -> bool {
        let mut w = on_world();
        let pool = idle(&w);
        let (e, d) = (pool[0], pool[1]);
        let farm = *w.buildings_of_kind(BuildingKind::Farm).first().expect("a Farm");
        if let Some(i) = w.comp_mut::<Identity>(e) {
            i.age_days = 64 * 120;
            i.born_tick = -(64 * 120 * TICKS_PER_DAY as i64);
        }
        // Not yet a parent of anyone (the seeding's grown children dropped).
        let kin: Vec<EntityId> = citysim::systems::demography::children_of_agent(&w, e);
        for k in kin {
            w.edge_entry(e, k).kind = RelKind::Acquaintance;
        }
        citysim::systems::demography::hire(&mut w, e, farm, citysim::Role::Farmer);
        let text = "let go: too sick to work".to_string();
        citysim::systems::economy::dismiss(&mut w, e, Some(farm), text);
        let old = w.comp::<Household>(e).and_then(|h| h.home);
        let dh = w.comp::<Household>(d).and_then(|h| h.home).expect("the daughter's home");
        assert_ne!(old, Some(dh), "apart");
        if daughter {
            if let Some(i) = w.comp_mut::<Identity>(d) {
                i.age_days = 35 * 120;
                i.born_tick = -(35 * 120 * TICKS_PER_DAY as i64);
            }
            let edge = w.edge_entry(e, d);
            edge.kind = RelKind::Parent;
            w.note_kin(e, d);
        }
        for _ in 0..7 {
            ages::daily(&mut w);
            if w.comp::<Household>(e).and_then(|h| h.home) == Some(dh) {
                return true;
            }
            w.tick += TICKS_PER_DAY;
        }
        false
    };
    assert!(run(true), "moved in with the daughter within the week");
    assert!(!run(false), "no daughter: stays put (the Mission cot is the homeless' path)");
}

/// A stabilised survivor maimed (p 1) works slower; with the coins a
/// prosthetic in the slot clears it.
#[test]
fn test_maimed_then_prosthetic() {
    let mut c = Config::load();
    c.ages.p_maimed = 1.0;
    let mut w = World::new(42, c);
    let a = idle(&w)[0];
    ages::on_rise(&mut w, a);
    assert!(w.jobs2.ages.maimed.contains_key(&a), "maimed");
    assert!(ages::work_mult(&w, a) < 1.0, "slower");
    assert!(ages::scale_ticks(&w, a, 60) < 60);
    coins(&mut w, a, 5000);
    ages::daily(&mut w);
    assert!(!w.jobs2.ages.maimed.contains_key(&a), "the prosthetic cleared it");
}

/// The seeded ages run to 75 under the switch, to 60 off; off, no School,
/// no state; a save round trip on.
#[test]
fn test_seeded_ages_off_and_save() {
    let w = on_world();
    let oldest =
        |w: &World| w.citizens().into_iter().filter_map(|a| w.comp::<Identity>(a)).map(|i| i.age_years()).max();
    assert!(oldest(&w).is_some_and(|y| y >= 61), "elders seeded");
    let mut c = Config::load();
    c.ages.enabled = false;
    let off = World::new(42, c);
    assert!(oldest(&off).is_some_and(|y| y < 60), "to 60 off");
    assert!(off.buildings_of_kind(BuildingKind::School).is_empty());
    assert!(off.jobs2.ages.is_default());
    let mut w = w;
    w.run_ticks(2 * TICKS_PER_DAY);
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text, "byte-identical round trip");
}

/// Phase 8 (§ 12): a homeless household's child is on the Street routine,
/// and after `street_take_days` of it the camp takes the child.
#[test]
fn test_street_child_taken_after_the_days() {
    let mut w = on_world();
    let home = w
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .copied()
        .find(|&h| w.comp::<citysim::Building>(h).is_some_and(|b| !b.derelict))
        .expect("a Home");
    let pool = idle(&w);
    let (mother, father) = (pool[0], pool[1]);
    for p in [mother, father] {
        w.set_home(p, Some(home));
    }
    let kid = citysim::systems::demography::spawn_child(&mut w, mother, father, home);
    for p in [mother, father, kid] {
        w.set_home(p, None);
    }
    let days = w.config.ages.street_take_days;
    for _ in 0..days.saturating_sub(1) {
        ages::daily(&mut w);
        w.tick += TICKS_PER_DAY;
    }
    assert_eq!(w.comp::<citysim::Child>(kid).map(|c| c.routine), Some(citysim::components::Routine::Street));
    assert!(w.stats.current.jobs2.as_ref().is_none_or(|j| j.street_takes == 0), "not before the days");
    ages::daily(&mut w);
    let camp = w.comp::<Household>(kid).and_then(|h| h.home);
    assert!(
        camp.and_then(|c| w.comp::<citysim::Building>(c)).is_some_and(|b| b.kind == BuildingKind::Camp),
        "the camp took the child"
    );
    assert!(w.stats.current.jobs2.as_ref().is_some_and(|j| j.street_takes == 1));
}

/// Phase-8 review finding 8: a behaviour chip ripped out is a maim with no
/// slot, and no prosthetic is a behaviour chip (on d2eea01 the maim kept
/// `Slot::Behaviour` and the midnight bought one as a "prosthetic").
#[test]
fn test_no_behaviour_slot_maim_or_prosthetic() {
    let mut w = on_world();
    let a = idle(&w)[0];
    ages::maim(&mut w, a, Some(citysim::components::Slot::Behaviour), "a ripped chip");
    assert_eq!(w.jobs2.ages.maimed.get(&a).map(|m| m.1), Some(None), "maimed, no limb slot");
    coins(&mut w, a, 5_000);
    ages::daily(&mut w);
    assert!(w.jobs2.ages.maimed.contains_key(&a), "no prosthetic bought for no limb");
}
