//! M4: memory, witnesses, law, jail.

use citysim::systems::{law, memory};
use citysim::{
    Brain, Building, BuildingKind, Config, Crime, Inventory, Job, Memory, MemoryEntry, MemoryKind, Needs, Personality,
    PlayerCommand, Position, Role, Sentence, Skills, Wallet, World, TICKS_PER_DAY,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

fn guard(w: &World) -> citysim::EntityId {
    w.citizens().into_iter().find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Guard)).expect("guard")
}

fn civilian(w: &World) -> citysim::EntityId {
    w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("civilian")
}

#[test]
fn test_witness_notice_formula() {
    let cfg = Config::load();
    assert!((law::notice_probability(&cfg.crime, 0.2, false) - 0.5).abs() < 1e-6);
    assert!((law::notice_probability(&cfg.crime, 0.2, true) - 0.8).abs() < 1e-6);
    assert!((law::notice_probability(&cfg.crime, 1.0, false) - 0.1).abs() < 1e-6);
}

/// A theft next to a guard, in the Market, at noon: the guard files a report
/// on sight (stealth 0 makes the roll certain) and the thief is jailed within
/// a day.
#[test]
fn test_theft_witnessed_by_guard_leads_to_jail_within_1_day() {
    let mut w = world(21);
    w.config.lod.force = Some(citysim::Lod::Full); // an arbitrary agent must be simulated in full
    let thief = civilian(&w);
    let g = guard(&w);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    w.run_ticks(700); // noon, day 0
    for id in [thief, g] {
        w.leave_building(id);
        w.enter_building(id, market);
        w.abort_plan(id);
    }
    w.comp_mut::<Skills>(thief).expect("skills").stealth = 0.0;
    w.comp_mut::<Personality>(thief).expect("p").courage = 0.2; // no contest
    let tile = w.comp::<Position>(thief).expect("pos").tile;
    law::raise_crime(&mut w, thief, None, Crime::Theft, tile);
    assert!(law::wanted(&w, thief), "a guard witness files a report immediately");
    assert!(w.comp::<Memory>(g).expect("mem").entries.iter().any(|e| e.kind == MemoryKind::SawCrime));

    let mut jailed_at = None;
    for t in 0..TICKS_PER_DAY {
        w.tick();
        if w.has::<Sentence>(thief) {
            jailed_at = Some(t);
            break;
        }
    }
    assert!(jailed_at.is_some(), "thief not jailed within a day");
    let s = w.comp::<Sentence>(thief).expect("sentence");
    assert_eq!(s.crime, Crime::Theft);
    assert!(w.comp::<Memory>(thief).expect("mem").entries.iter().any(|e| e.kind == MemoryKind::WasArrested));
    assert!(w.crime_reports().iter().filter(|r| r.suspect == thief).all(|r| r.resolved));
    assert_eq!(w.comp::<Position>(thief).expect("pos").building, w.building_of_kind(BuildingKind::Jail));
}

#[test]
fn test_theft_unwitnessed_no_arrest() {
    let mut w = world(22);
    let thief = civilian(&w);
    // alone on the far east road, everyone else indoors; night for the small radius
    w.tick = 100;
    w.leave_building(thief);
    w.comp_mut::<Position>(thief).expect("pos").tile = citysim::TilePos { x: 79, y: 40 };
    let tile = w.comp::<Position>(thief).expect("pos").tile;
    let nearby = w
        .citizens()
        .into_iter()
        .filter(|&o| o != thief)
        .filter(|&o| w.comp::<Position>(o).is_some_and(|p| p.building.is_none() && law::chebyshev(p.tile, tile) <= 8))
        .count();
    assert_eq!(nearby, 0, "scenario needs an empty street");
    law::raise_crime(&mut w, thief, None, Crime::Theft, tile);
    assert!(!law::wanted(&w, thief));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.crime_reports().iter().all(|r| r.suspect != thief), "{:?}", w.crime_reports());
    assert!(!w.has::<Sentence>(thief));
    // the unwitnessed thief got better at it
    assert!(w.comp::<Skills>(thief).expect("skills").stealth > 0.1);
}

#[test]
fn test_memory_cap_24_evicts_lowest_weight() {
    let mut mem = Memory::default();
    let now = 100 * TICKS_PER_DAY;
    for i in 0..30u64 {
        // distinct subjects so nothing merges; the 7th entry is the lightest: old and faint
        let (tick, salience) = if i == 7 { (now - 20 * TICKS_PER_DAY, 0.05) } else { (now - i * 60 * 2, 0.5) };
        let entry = MemoryEntry {
            kind: MemoryKind::Ate,
            subject: Some(citysim::EntityId { index: i as u32, generation: 0 }),
            tick,
            salience,
            valence: 0.0,
            second_hand: false,
            crime: None,
        };
        memory::insert(&mut mem, entry, now, 24, 7.0);
    }
    assert_eq!(mem.entries.len(), 24);
    assert!(!mem.entries.iter().any(|e| e.subject == Some(citysim::EntityId { index: 7, generation: 0 })));
    // rapid duplicates merge instead of growing the list
    let dup = |t| MemoryEntry {
        kind: MemoryKind::WasRobbed,
        subject: Some(citysim::EntityId { index: 99, generation: 0 }),
        tick: t,
        salience: 0.6,
        valence: -0.6,
        second_hand: false,
        crime: None,
    };
    memory::insert(&mut mem, dup(now), now, 24, 7.0);
    memory::insert(&mut mem, dup(now + 10), now + 10, 24, 7.0);
    assert_eq!(mem.entries.iter().filter(|e| e.kind == MemoryKind::WasRobbed).count(), 1);
}

#[test]
fn test_memory_half_life() {
    let mut w = world(23);
    w.config.lod.force = Some(citysim::Lod::Full); // Statistical memories keep only three kinds
    let id = civilian(&w);
    w.comp_mut::<Memory>(id).expect("mem").entries.clear();
    w.remember(id, MemoryKind::Grief, None, 1.0, -0.5, false);
    for _ in 0..7 {
        w.run_ticks(TICKS_PER_DAY);
    }
    let s =
        w.comp::<Memory>(id).expect("mem").entries.iter().find(|e| e.kind == MemoryKind::Grief).expect("kept").salience;
    assert!((s - 0.5).abs() <= 0.01, "salience after 7 daily decays: {s}");
}

#[test]
fn test_sentence_mult_scales_release_tick() {
    let mut w = world(24);
    assert_eq!(law::sentence_ticks(&w, Crime::Theft), 3 * TICKS_PER_DAY);
    w.levers.sentence_mult = 2.0;
    assert_eq!(law::sentence_ticks(&w, Crime::Theft), 6 * TICKS_PER_DAY);
    assert_eq!(law::sentence_ticks(&w, Crime::Murder), 60 * TICKS_PER_DAY);
    w.levers.sentence_mult = 0.5;
    assert_eq!(law::sentence_ticks(&w, Crime::Theft), 2 * TICKS_PER_DAY, "rounded up");
}

#[test]
fn test_jail_full_theft_becomes_fine() {
    let mut w = world(25);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(17).collect();
    let until = w.tick + 3 * TICKS_PER_DAY;
    for &id in &ids[..16] {
        law::sentence(&mut w, id, Crime::Theft, until, jail);
    }
    assert_eq!(w.with::<Sentence>().len(), 16);
    let thief = ids[16];
    let g = guard(&w);
    w.comp_mut::<Wallet>(thief).expect("wallet").coins = 20;
    let price = w.mean_price();
    let treasury = w.treasury().expect("t").coins;
    law::file_report(&mut w, Crime::Theft, thief, Some(g));
    law::jail_suspect(&mut w, g, thief);
    assert!(!w.has::<Sentence>(thief), "fined, not jailed");
    assert_eq!(w.comp::<Wallet>(thief).expect("wallet").coins, 20 - 2 * price);
    assert_eq!(w.treasury().expect("t").coins, treasury + 2 * price);
    assert!(w
        .comp::<Memory>(thief)
        .expect("mem")
        .entries
        .iter()
        .any(|e| e.kind == MemoryKind::Paid && e.valence < 0.0));
    assert!(!law::wanted(&w, thief));
    // broke: released unpunished with a lawfulness drop
    let thief2 = w
        .citizens()
        .into_iter()
        .find(|&id| !w.has::<Job>(id) && !w.has::<Sentence>(id) && id != thief)
        .expect("free civilian");
    w.comp_mut::<Wallet>(thief2).expect("wallet").coins = 0;
    let before = w.comp::<Personality>(thief2).expect("p").lawfulness;
    law::file_report(&mut w, Crime::Theft, thief2, Some(g));
    law::jail_suspect(&mut w, g, thief2);
    assert!(!w.has::<Sentence>(thief2));
    assert!(w.comp::<Personality>(thief2).expect("p").lawfulness < before);
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Unpunished && e.actors.contains(&thief2)));
}

#[test]
fn test_jail_full_bumps_a_less_severe_prisoner() {
    let mut w = world(27);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(17).collect();
    let until = w.tick + 5 * TICKS_PER_DAY;
    for &id in &ids[..16] {
        law::sentence(&mut w, id, Crime::Dealing, until, jail);
    }
    assert_eq!(w.with::<Sentence>().len(), 16);
    let killer = ids[16];
    let g = guard(&w);
    law::file_report(&mut w, Crime::Murder, killer, Some(g));
    law::jail_suspect(&mut w, g, killer);
    assert_eq!(w.comp::<Sentence>(killer).map(|s| s.crime), Some(Crime::Murder), "murderer sentenced");
    let dealers = ids[..16].iter().filter(|&&id| w.has::<Sentence>(id)).count();
    assert_eq!(dealers, 15, "one dealer released");
    assert_eq!(w.with::<Sentence>().len(), 16);
}

#[test]
fn test_jail_full_of_murderers_does_not_bump_for_a_thief() {
    let mut w = world(28);
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let ids: Vec<_> = w.citizens().into_iter().filter(|&id| !w.has::<Job>(id)).take(17).collect();
    let until = w.tick + 5 * TICKS_PER_DAY;
    for &id in &ids[..16] {
        law::sentence(&mut w, id, Crime::Murder, until, jail);
    }
    let thief = ids[16];
    let g = guard(&w);
    w.comp_mut::<Wallet>(thief).expect("wallet").coins = 0;
    law::file_report(&mut w, Crime::Theft, thief, Some(g));
    law::jail_suspect(&mut w, g, thief);
    assert!(!w.has::<Sentence>(thief), "thief released or fined");
    assert!(ids[..16].iter().all(|&id| w.has::<Sentence>(id)), "no murderer bumped");
}

#[test]
fn test_player_arrest_bypasses_witness() {
    let mut w = world(26);
    let id = civilian(&w);
    w.push_command(PlayerCommand::Arrest(id));
    w.tick();
    let s = w.comp::<Sentence>(id).expect("sentenced on the next tick");
    assert_eq!(s.until_tick, 3 * TICKS_PER_DAY, "applied at the start of tick 0");
    assert_eq!(w.comp::<Position>(id).expect("pos").building, w.building_of_kind(BuildingKind::Jail));
    assert!(w
        .comp::<Memory>(id)
        .expect("mem")
        .entries
        .iter()
        .any(|e| e.kind == MemoryKind::WasArrested && e.subject.is_none()));
    // and the inmate is fed and released on time
    w.comp_mut::<Needs>(id).expect("needs").hunger = 0.3;
    w.run_ticks(3 * TICKS_PER_DAY + 2);
    assert!(!w.has::<Sentence>(id), "released");
    assert!(w.comp::<Needs>(id).expect("needs").hunger > 0.3, "fed in jail");
    assert!((w.comp::<Needs>(id).expect("needs").belonging - 0.2).abs() < 0.01, "belonging reset on release");
    w.push_command(PlayerCommand::Release(id));
    w.tick();
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::PlayerActionFailed));
    let _ = w.comp::<Inventory>(id);
    let _ = w.comp::<Brain>(id);
    let _ = w.comp::<Building>(w.building_of_kind(BuildingKind::Jail).expect("jail"));
}

/// M10 5c: a guard with a warrant of their own never chases or cuffs
/// themselves (one who did sat in their own cuffs, with no escort to end it,
/// and starved).
#[test]
fn test_guard_never_arrests_self() {
    let mut w = world(31);
    let g = guard(&w);
    law::file_report(&mut w, Crime::Theft, g, None);
    let tile = w.comp::<Position>(g).expect("pos").tile;
    let tick = w.tick;
    w.last_seen.insert(g, (tile, tick));
    assert!(law::is_located_suspect(&w, g), "the guard is a located suspect");
    assert!(!law::located_suspects(&w, Some(law::Pursuer { tile, guard: g })).contains(&g));
    assert!(law::located_suspects(&w, None).contains(&g), "the city-wide list still names them");
    assert!(!law::arrest(&mut w, g, g));
    assert!(w.comp::<Brain>(g).expect("brain").cuffed_by.is_none());
}

/// M10 5c: the shift clock owes a guard's wages: once to a guard on law duty
/// for the shift, whatever they do at its end; nothing to one who slept
/// through it; the day to one who completed it before going to bed.
#[test]
fn test_guard_on_duty_at_shift_end_is_owed_the_day() {
    use citysim::GoalKind;
    let mut w = world(32);
    let g = guard(&w);
    let job = w.comp::<Job>(g).expect("job").clone();
    // A segment end that really ends the shift (not 1440 continued by a 0 start).
    let end = job.shifts.iter().map(|&(_, e)| e).find(|&e| !job.on_shift(e % 1440)).expect("an end");
    let mut day = 1;
    let key = loop {
        let tick = day * TICKS_PER_DAY + u64::from(end % 1440) + if end == 1440 { TICKS_PER_DAY } else { 0 };
        let key = job.shift_key_at(tick - 1);
        if citysim::exec::routine::is_workday(key) {
            w.tick = tick;
            break key;
        }
        day += 1;
    };
    let before = w.comp::<Job>(g).expect("job").days_unpaid;
    // On duty all shift, eating at its end.
    w.comp_mut::<Job>(g).expect("job").duty_ticks = 400;
    w.comp_mut::<Brain>(g).expect("brain").current_goal = Some(GoalKind::Eat);
    law::run(&mut w);
    let j = w.comp::<Job>(g).expect("job");
    assert_eq!(j.days_unpaid, before + 1, "one day owed");
    assert_eq!(j.last_shift_day, Some(key));
    // The next tick is not a shift end; a GuardJail or PatrolLeg finishing
    // now (`end_shift`) owes a guard nothing more.
    w.tick += 1;
    law::run(&mut w);
    citysim::exec::actions::end_shift(&mut w, g);
    assert_eq!(w.comp::<Job>(g).expect("job").days_unpaid, before + 1, "not owed twice");

    // Another guard, asleep when the same shift ends, is owed nothing.
    let other = w
        .citizens()
        .into_iter()
        .find(|&o| o != g && w.comp::<Job>(o).is_some_and(|j| j.role == Role::Guard && j.shifts == job.shifts))
        .expect("a guard on the same shift");
    let before = w.comp::<Job>(other).expect("job").days_unpaid;
    w.comp_mut::<Brain>(other).expect("brain").current_goal = Some(GoalKind::Sleep);
    w.comp_mut::<Job>(other).expect("job").last_shift_day = None;
    w.comp_mut::<Job>(other).expect("job").duty_ticks = 60;
    w.tick -= 1;
    law::run(&mut w);
    assert_eq!(w.comp::<Job>(other).expect("job").days_unpaid, before);

    // A guard who completed the shift (five legs) and went to bed is owed it.
    let third = w
        .citizens()
        .into_iter()
        .find(|&o| {
            o != g && o != other && w.comp::<Job>(o).is_some_and(|j| j.role == Role::Guard && j.shifts == job.shifts)
        })
        .expect("a third guard on the same shift");
    let before = w.comp::<Job>(third).expect("job").days_unpaid;
    w.comp_mut::<Brain>(third).expect("brain").current_goal = Some(GoalKind::Sleep);
    w.comp_mut::<Job>(third).expect("job").last_shift_day = Some(key);
    w.comp_mut::<Job>(third).expect("job").duty_ticks = 0;
    law::run(&mut w);
    assert_eq!(w.comp::<Job>(third).expect("job").days_unpaid, before + 1);
}
