//! Lives at work phase 5 (docs/LIVES_AT_WORK.md § 2.4, § 2.2's civic clerk,
//! § 3.6's `Decided`, § 9.1): the tower and the brokers. Unit tests in the
//! shipped seed-42 city with the preconditions set by hand. Each failed on
//! the base `f5160ee` (no `service::decide`, no walk-in queue, no
//! `World::meeting_tables`, no `Story.byline`, no `licence_clerk`).
mod common;

use citysim::components::{
    Brain, BuildingKind, Corp, CorpOrder, CorpOrderScore, Job, Lod, Memory, MemoryKind, Niche, Personality,
};
use citysim::systems::{gossip, news, service};
use citysim::word::Deed;
use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY, TICKS_PER_HOUR};

/// A whole-day shift whose key is the day (a `(0, 1440)` shift keys to the day before).
const ALL_DAY: (u16, u16) = (1, 1440);

fn served_memories(w: &World, id: EntityId) -> Vec<EntityId> {
    w.comp::<Memory>(id)
        .map(|m| m.entries.iter().filter(|e| e.kind == MemoryKind::Served).filter_map(|e| e.subject).collect())
        .unwrap_or_default()
}

/// A corp with a living exec, and the exec.
fn corp_with_exec(w: &World) -> (EntityId, EntityId) {
    w.corps()
        .into_iter()
        .find_map(|c| {
            let e = w.comp::<Corp>(c)?.exec?;
            (citysim::systems::law::living(w, e) && citysim::systems::life::hq_of(w, c, e).is_some()).then_some((c, e))
        })
        .expect("a corp with an exec and an HQ")
}

fn score(order: CorpOrder, s: f32) -> CorpOrderScore {
    CorpOrderScore { order, niche: Niche::Food, score: s, considerations: Vec::new() }
}

fn set_traits(w: &mut World, id: EntityId, courage: f32, loyalty: f32) {
    let p = w.comp_mut::<Personality>(id).expect("personality");
    p.courage = courage;
    p.loyalty = loyalty;
}

/// § 2.4: the decision picks between the two best orders only within
/// `decision_margin`: a corp whose best clears the margin is untouched;
/// within it, a brave exec takes the expansive and a loyal one the defensive.
#[test]
fn test_decision_only_within_the_margin_brave_expands_loyal_hunkers() {
    let mut w = common::world();
    assert!(service::tower_on(&w));
    let (corp, exec) = corp_with_exec(&w);
    let margin = w.config.service.decision_margin;
    // Clear of the margin: untouched.
    let mut wide = vec![score(CorpOrder::Grow, 0.60 + 2.0 * margin), score(CorpOrder::Hunker, 0.60)];
    let before = wide.clone();
    assert_eq!(service::decide(&w, corp, &mut wide), None);
    assert_eq!(wide, before, "a best clear of the margin is the scores' own");
    // Within it: the brave exec's call is Grow, the loyal one's Hunker.
    set_traits(&mut w, exec, 1.0, 0.0);
    let mut close = vec![score(CorpOrder::Hunker, 0.60), score(CorpOrder::Grow, 0.60 - margin / 2.0)];
    assert_eq!(service::decide(&w, corp, &mut close), Some(exec));
    assert_eq!(close[0].order, CorpOrder::Grow, "courage leans expansive");
    set_traits(&mut w, exec, 0.0, 1.0);
    let mut close = vec![score(CorpOrder::Grow, 0.60), score(CorpOrder::Hunker, 0.60 - margin / 2.0)];
    assert_eq!(service::decide(&w, corp, &mut close), Some(exec));
    assert_eq!(close[0].order, CorpOrder::Hunker, "loyalty leans defensive");
    // Two expansive orders are not a call.
    let mut same = vec![score(CorpOrder::Grow, 0.60), score(CorpOrder::Research, 0.60)];
    assert_eq!(service::decide(&w, corp, &mut same), None);
    // Off: no call.
    let mut off = common::world_with(|c| c.strip_tower());
    let (corp, _) = corp_with_exec(&off);
    let mut close = vec![score(CorpOrder::Grow, 0.60), score(CorpOrder::Hunker, 0.60)];
    assert_eq!(service::decide(&off, corp, &mut close), None);
    off.run_ticks(1);
}

/// A standing Fixer with a forger on staff, and the forger.
fn fixer_and_forger(w: &World) -> (EntityId, EntityId) {
    let role = w.config.trade_role("forger").expect("the forger trade");
    w.buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .find_map(|&b| {
            let f = w.staff_of(b).iter().copied().find(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == role))?;
            Some((b, f))
        })
        .expect("a Fixer with a forger")
}

/// A Statistical adult with a wallet, out of any gang, unwanted, unpapered.
fn buyer(w: &mut World, coins: i64) -> EntityId {
    let a = common::free_adults(w)
        .into_iter()
        .find(|&a| {
            w.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical)
                && !w.has::<Job>(a)
                && !w.jobs2.gray.papers.contains_key(&a)
        })
        .expect("a jobless Statistical adult");
    w.comp_mut::<citysim::Wallet>(a).expect("wallet").coins = coins;
    citysim::systems::law::file_report(w, citysim::Crime::Theft, a, None);
    assert!(w.open_suspects().any(|s| s == a), "the buyer is wanted");
    a
}

/// § 2.4 (fixer): a paper sale lands in the Fixer's open hours, never at
/// 00:00: queued at midnight, served at the forger's first shift hour (a
/// Statistical buyer credited at the hour); off, the midnight sale as main.
#[test]
fn test_paper_sale_is_a_walk_in_in_open_hours_never_at_midnight() {
    // Day 1's midnight (the Fixers are staffed on day 0's).
    let mut w = common::world_at(TICKS_PER_DAY);
    let (fixer, forger) = fixer_and_forger(&w);
    let a = buyer(&mut w, 500);
    w.run_ticks(1); // the midnight pass of day 1
    assert!(!w.jobs2.gray.papers.contains_key(&a), "no sale at 00:00");
    assert!(w.walk_ins.iter().any(|x| x.buyer == a && x.fixer == fixer), "queued for {}", w.name_of(fixer));
    // A wanted agent may be given a body at midnight (then he walks in,
    // `walk_in_arrived`): held Statistical here, credited at the hour.
    citysim::systems::lod::set_lod(&mut w, a, Lod::Statistical);
    w.visits.remove(&a);
    // The forger at his desk from 09:00 (a Full body inside, a day shift).
    if let Some(j) = w.comp_mut::<Job>(forger) {
        j.shifts = vec![(540, 1080)];
    }
    citysim::systems::lod::set_lod(&mut w, forger, Lod::Full);
    // 08:00: nobody on duty yet.
    w.tick = TICKS_PER_DAY + 8 * TICKS_PER_HOUR;
    w.enter_building(forger, fixer);
    service::hourly(&mut w);
    assert!(!w.jobs2.gray.papers.contains_key(&a), "closed at 08:00");
    w.tick = TICKS_PER_DAY + 9 * TICKS_PER_HOUR;
    w.enter_building(forger, fixer);
    assert!(service::present(&w, forger, fixer), "the forger at his desk at 09:00");
    service::hourly(&mut w);
    let p = w.jobs2.gray.papers.get(&a).expect("sold at the first open hour");
    assert!(p.fake && p.forger == Some(forger));
    assert!(!w.walk_ins.iter().any(|x| x.buyer == a), "served, off the queue");
    let e =
        w.events.iter().rev().find(|e| e.kind == EventKind::Served && e.actors.contains(&a)).expect("the sale's event");
    assert_eq!(e.tick % TICKS_PER_DAY, 9 * TICKS_PER_HOUR, "{}", e.text);
    assert!(e.text.contains("papers"), "{}", e.text);
    assert!(regular_of(&w, forger, a), "the forger met the buyer");
    // Off: the midnight sale, as main.
    let mut off = common::world_with(|c| c.strip_tower());
    off.run_ticks(TICKS_PER_DAY);
    let b = buyer(&mut off, 500);
    off.run_ticks(1);
    assert!(off.jobs2.gray.papers.contains_key(&b), "off sells at midnight");
    assert!(off.walk_ins.is_empty());
}

fn regular_of(w: &World, server: EntityId, who: EntityId) -> bool {
    service::regulars_of(w, server).iter().any(|r| r.who == who)
}

/// § 2.4 (exec): a Meeting hour meets up to `meeting_attendees` graded
/// staff present at the Hq and no labourer; both sides remember it.
#[test]
fn test_meeting_hour_meets_graded_staff_and_no_labourer() {
    let mut w = common::world();
    let (corp, exec) = corp_with_exec(&w);
    let hq = citysim::systems::life::hq_of(&w, corp, exec).expect("hq");
    // Day 1, 10:00, a workday: the exec a body at the table.
    w.tick = TICKS_PER_DAY + 10 * TICKS_PER_HOUR;
    citysim::systems::lod::set_lod(&mut w, exec, Lod::Full);
    w.enter_building(exec, hq);
    let staff: Vec<EntityId> = w.staff_of(hq).to_vec();
    assert!(staff.len() >= 2, "the HQ is staffed ({})", staff.len());
    let mut graded = Vec::new();
    let mut labour = Vec::new();
    for (i, &s) in staff.iter().enumerate() {
        let base = w.comp::<Job>(s).map_or(0, |j| citysim::systems::classpay::base_grade(&w, j.employer, j.role));
        if let Some(j) = w.comp_mut::<Job>(s) {
            j.shifts = vec![ALL_DAY];
            j.grade = u8::from(i % 2 == 0);
        }
        citysim::systems::lod::set_lod(&mut w, s, Lod::Full);
        w.enter_building(s, hq);
        if !service::present(&w, s, hq) {
            continue;
        }
        if i % 2 == 0 || base >= 1 {
            graded.push(s);
        } else {
            labour.push(s);
        }
    }
    assert!(!graded.is_empty(), "graded staff present");
    service::hourly(&mut w);
    let table = w.meeting_tables.get(&corp).expect("a Meeting with attendees").to_vec();
    let cap = usize::from(w.config.service.meeting_attendees);
    assert_eq!(table.len(), graded.len().min(cap), "up to {cap} of {} graded", graded.len());
    for a in &table {
        assert!(graded.contains(a), "{} is graded", w.name_of(*a));
        assert!(served_memories(&w, *a).contains(&exec), "{} remembers the exec", w.name_of(*a));
    }
    for l in &labour {
        assert!(!table.contains(l), "a labourer is not at the table");
    }
    assert!(served_memories(&w, exec).iter().any(|s| table.contains(s)), "the exec remembers the table");
}

/// § 2.4 (reporter): a story run from an interviewed deed carries that
/// reporter's byline, and the event names him.
#[test]
fn test_story_from_an_interviewed_deed_carries_the_byline() {
    let mut w = common::world();
    let feed = news::feeds(&w)
        .into_iter()
        .find(|&f| {
            w.staff_of(f).iter().any(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == citysim::Role::Reporter))
                && news::feed_state(&w, f).is_some_and(|s| s.covers != 0)
        })
        .expect("a staffed Feed covering a district");
    let reporters: Vec<EntityId> = w
        .staff_of(feed)
        .iter()
        .copied()
        .filter(|&s| w.comp::<Job>(s).is_some_and(|j| j.role == citysim::Role::Reporter))
        .collect();
    let reporter = *reporters.last().expect("a reporter");
    let covers = news::feed_state(&w, feed).expect("feed").covers;
    let d = (0..32u8).find(|d| covers & (1 << d) != 0).expect("a covered district");
    let civ = common::civilians(&w);
    let (actor, victim) = (civ[3], civ[7]);
    w.tick = 10 * TICKS_PER_HOUR;
    gossip::post_deed(&mut w, citysim::components::DistrictId(d), Deed::Killed, Some(actor), Some(victim));
    let key = (Deed::Killed, actor, Some(victim), 0u64);
    w.beats.insert(
        reporter,
        news::Beat { shift: 0, stops: Default::default(), legs: 0, interviewed: std::iter::once(key).collect() },
    );
    news::daily(&mut w);
    let s = w
        .stories
        .iter()
        .find(|s| s.feed == feed && s.deed == Deed::Killed && s.deed_actor() == actor)
        .expect("the Feed ran the killing");
    assert_eq!(s.byline, Some(reporter), "the interviewing reporter's byline");
    let name = w.name_of(reporter);
    assert!(
        w.events.iter().any(|e| e.kind == EventKind::Story && e.text.contains(&format!("by {name}"))),
        "the story event names {name}"
    );
    assert_eq!(w.bylines.get(&reporter), Some(&1));
    assert!(w.beats.get(&reporter).is_none_or(|b| b.interviewed.is_empty()), "the interviews are spent");
}

/// § 2.2 (civic clerk): a licence names a civic clerk at the Hall (the
/// lowest hash), and the sale is his `Client` encounter.
#[test]
fn test_licence_names_the_clerk() {
    let mut w = common::world_at(3 * TICKS_PER_DAY);
    eprintln!("clerks on the books at day 3: {}", citysim::systems::industry::clerks_on_books(&w));
    let a = common::civilians(&w)[5];
    let (clerk, hall) = service::licence_clerk(&w, a).expect("a civic clerk at a Hall");
    let role = w.config.trade_role("civic_clerk").expect("civic_clerk");
    assert_eq!(w.comp::<Job>(clerk).map(|j| (j.role, j.employer)), Some((role, Some(hall))));
    service::licence_sold(&mut w, clerk, hall, a, 10);
    assert!(regular_of(&w, clerk, a), "the clerk's ledger names the buyer");
    let off = common::world_with(|c| c.strip_tower());
    assert_eq!(service::licence_clerk(&off, a), None);
}

/// Off: three days write nothing of the phase (no table, beat, byline,
/// walk-in, counter, decision or tower row), and the config carries no
/// tower key.
#[test]
fn test_off_city_writes_nothing_in_three_days() {
    let mut w = common::world_with(|c| c.strip_tower());
    assert!(!service::tower_on(&w));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.meeting_tables.is_empty() && w.beats.is_empty() && w.bylines.is_empty() && w.walk_ins.is_empty());
    assert!(w.stories.iter().all(|s| s.byline.is_none()));
    for r in &w.stats.history {
        let j = r.jobs2.as_ref().expect("jobs2 on");
        assert_eq!(
            (j.meetings_held, j.decisions, j.beat_stops, j.interviews, j.bylines, j.walk_ins_served, j.walk_ins_waited),
            (0, 0, 0, 0, 0, 0, 0)
        );
        assert_eq!((j.contract_encounters, j.fixer_collections, j.licences_named, j.bribes_named), (0, 0, 0, 0));
        assert_eq!(j.served[citysim::components::ServeKind::Attendee as usize], 0);
        assert_eq!(j.served[citysim::components::ServeKind::Source as usize], 0);
    }
    assert!(!w.events.iter().any(|e| e.text.contains(" decided ") || e.text.contains(" papers at ")));
    let text = ron::to_string(&w.config).expect("config");
    assert!(!text.contains("decided") && !text.contains("meeting_attendees") && !text.contains("beat_stops"));
    // The env device's config matches.
    let mut on = Config::load();
    assert!(on.service.tower && on.serves.iter().any(|r| r.tower));
    on.strip_tower();
    assert!(!on.serves.iter().any(|r| r.tower));
}
