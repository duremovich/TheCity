//! M15 phase 4 (plan 4.7): the Feeds. A Feed is a building whose staff turn
//! the day's pool entries (records of earlier events) into story records
//! posted back into the pools; Spin is a corp paying a Feed to run a known
//! record about a rival or to drop records about itself; press is a number
//! summed from the story records an agent holds. Every test here reads
//! counters, coin transfers and those records.

use citysim::components::DistrictId;
use citysim::systems::{classes, demography, law, memory, news, ownership, reputation};
use citysim::word::{Deed, PoolEntry, Story};
use citysim::{
    BuildingKind, Config, Corp, EntityId, EventKind, Job, MemoryEntry, MemoryKind, PlayerCommand, World, TICKS_PER_DAY,
};

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.name == name)).expect(name)
}

fn civic_wire(w: &World) -> EntityId {
    news::all_feeds(w).into_iter().find(|&f| news::is_city_feed(w, f)).expect("the Civic Wire")
}

fn nutrix_now(w: &World) -> EntityId {
    let n = corp_named(w, "Nutrix");
    news::all_feeds(w).into_iter().find(|&f| w.owner_of(f) == Some(n)).expect("Nutrix Now")
}

/// Living adults in no gang, ascending.
fn plain_adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&id| law::living(w, id) && demography::is_adult(w, id) && w.gang_of(id).is_none())
        .collect()
}

fn clear_pools(w: &mut World) {
    for p in w.rumours.iter_mut() {
        p.entries.clear();
    }
}

fn entry(deed: Deed, actor: EntityId, object: Option<EntityId>, tick: u64, d: u8) -> PoolEntry {
    PoolEntry {
        deed,
        actor: Some(actor),
        object,
        tick,
        hops: 0,
        reach: 1.0,
        hole: None,
        story: None,
        kin: Default::default(),
        told: Default::default(),
        district: DistrictId(d),
        press: 0,
    }
}

fn heard(subject: EntityId, deed: Deed, object: Option<EntityId>, tick: u64, press: i8) -> MemoryEntry {
    MemoryEntry {
        subject: Some(subject),
        salience: 0.8,
        valence: -0.5,
        second_hand: true,
        deed: Some(deed),
        object,
        hops: 1,
        conf: 0.6,
        press,
        ..MemoryEntry::blank(MemoryKind::Rumour, tick)
    }
}

fn apply(w: &mut World, cmd: PlayerCommand) {
    w.push_command(cmd);
    w.apply_commands();
}

fn stories_by(w: &World, feed: EntityId) -> Vec<Story> {
    w.stories.iter().filter(|s| s.feed == feed).cloned().collect()
}

fn reach_of(w: &World, f: EntityId) -> f32 {
    news::feed_state(w, f).map_or(0.0, |s| s.reach)
}

#[test]
fn test_feeds_seeded_with_owner_district_and_vacancies() {
    let w = World::new(42, Config::load());
    let feeds = news::all_feeds(&w);
    assert_eq!(feeds.len(), 2, "the Civic Wire and Nutrix Now");
    let civic = civic_wire(&w);
    let nn = nutrix_now(&w);
    let dname = |b: EntityId| w.districts[w.district_of_building(b).index()].name.clone();
    assert_eq!(dname(civic), "Civic");
    assert_eq!(dname(nn), "Mid West");
    assert_eq!(news::feed_name(&w, civic), news::CIVIC_WIRE);
    assert_eq!(news::feed_name(&w, nn), "Nutrix Now");
    for f in [civic, nn] {
        assert_eq!(w.comp::<citysim::Building>(f).map(|b| b.kind), Some(BuildingKind::Feed));
        let staff = w.config.buildings.feed.staff as usize;
        assert_eq!(w.vacancies.get(&f).map_or(0, Vec::len), staff, "a Reporter vacancy per [buildings] feed.staff");
    }
}

#[test]
fn test_feed_posts_at_reach_and_skips_buried() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    apply(&mut w, PlayerCommand::SetPressLicence(false));
    let civic = civic_wire(&w);
    let ads = plain_adults(&w);
    let (a, b, c, d) = (ads[0], ads[1], ads[2], ads[3]);
    // A buried actor is skipped; a free one runs.
    let until = w.tick + 2 * TICKS_PER_DAY;
    w.comp_mut::<citysim::Building>(civic).unwrap().feed.as_mut().unwrap().buried.push_back((c, until));
    let now = w.tick;
    citysim::systems::gossip::post(&mut w, DistrictId(0), entry(Deed::Killed, a, Some(b), now, 0));
    citysim::systems::gossip::post(&mut w, DistrictId(0), entry(Deed::Killed, c, Some(d), now, 0));
    news::daily(&mut w);
    let run = stories_by(&w, civic);
    assert!(run.iter().any(|s| s.deed == Deed::Killed && s.object == Some(b)), "the free killing runs: {run:?}");
    assert!(run.iter().all(|s| s.actor != c), "the buried actor is skipped: {run:?}");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Story));
    // Posted into another covered district at the Feed's reach × story_reach, hops 1, tagged.
    let r = reach_of(&w, civic) * w.config.news.story_reach;
    let other = &w.rumours[1].entries;
    let story = other.iter().find(|e| e.story == Some(civic) && e.object == Some(b)).expect("the story in district 1");
    assert!((story.reach - r).abs() < 1e-5, "reach {} vs {r}", story.reach);
    assert_eq!(story.hops, 1);
    assert!(story.press < 0, "a killing runs with a negative slant");
    assert!(!other.iter().any(|e| e.actor == Some(c)), "nothing about the buried actor");
}

#[test]
fn test_spin_plants_rival_misdeed_and_charges_plant() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    let nutrix = corp_named(&w, "Nutrix");
    let vatra = corp_named(&w, "Vatra");
    let rival_exec = w.comp::<Corp>(vatra).and_then(|c| c.exec).expect("Vatra's exec");
    let exec = w.comp::<Corp>(nutrix).and_then(|c| c.exec).expect("Nutrix's exec");
    let victim = plain_adults(&w).into_iter().find(|&v| v != exec && v != rival_exec).unwrap();
    let now = w.tick;
    memory::hear_entry(&mut w, exec, heard(rival_exec, Deed::Robbed, Some(victim), now, 0));
    // Spin is a side spend: every corp whose gate is open spins.
    w.config.news.spin_min = 0.0;
    let civic = civic_wire(&w);
    let w2 = &mut w;
    let total = ownership::total_coins(w2);
    let t0 = w2.treasury().map_or(0, |t| t.coins);
    let ads0 = w2.stats.current.word.flow_ads;
    news::daily(w2);
    assert_eq!(ownership::total_coins(w2), total, "a plant moves coins, never makes them");
    assert_eq!(w2.stats.current.word.planted, 1);
    let price = (w2.config.news.plant_price as f32 * reach_of(w2, civic)).round() as i64;
    assert!(price > 0);
    assert_eq!(w2.stats.current.word.flow_plant, price, "flow_plant is the payment");
    // The Civic Wire is the largest Feed Nutrix does not own: the Treasury is paid.
    let ads_to_city = w2.stats.current.word.flow_ads - ads0;
    let t1 = w2.treasury().map_or(0, |t| t.coins);
    assert!(t1 - t0 >= price, "the Treasury took the plant ({}) and its ads ({ads_to_city})", t1 - t0);
    let s = w2.stories.iter().find(|s| s.paid_by == Some(nutrix)).expect("the planted story");
    assert_eq!((s.feed, s.deed, s.slant), (civic, Deed::Robbed, -1.0));
    assert!(w2.events.iter().any(|e| e.kind == EventKind::Planted && e.actors.first() == Some(&nutrix)));
}

#[test]
fn test_spin_buries_own_story_for_bury_days() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    let nutrix = corp_named(&w, "Nutrix");
    let exec = w.comp::<Corp>(nutrix).and_then(|c| c.exec).expect("exec");
    let civic = civic_wire(&w);
    let now = w.tick;
    w.stories.push_back(Story {
        id: 0,
        feed: civic,
        deed: Deed::Evicted,
        actor: exec,
        source: None,
        object: None,
        tick: now,
        slant: -0.3,
        paid_by: None,
    });
    w.next_story_id = 1;
    w.config.news.spin_min = 0.0;
    news::daily(&mut w);
    let bury = w.config.news.bury_days as u64 * TICKS_PER_DAY;
    let st = news::feed_state(&w, civic).unwrap();
    assert!(st.buried.iter().any(|&(a, until)| a == exec && until == now + bury), "{:?}", st.buried);
    assert_eq!(w.stats.current.word.buried, 1);
    let price = (w.config.news.bury_price as f32 * reach_of(&w, civic)).round() as i64;
    assert_eq!(w.stats.current.word.flow_plant, price);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Buried && e.actors.first() == Some(&nutrix)));
    // While buried, the Civic Wire drops the exec's deeds.
    let victim = plain_adults(&w)[0];
    citysim::systems::gossip::post(&mut w, DistrictId(0), entry(Deed::Killed, exec, Some(victim), now, 0));
    news::daily(&mut w);
    assert!(w.stories.iter().all(|s| !(s.feed == civic && s.actor == exec && s.deed == Deed::Killed)));
    // Past bury_days the bury lapses.
    w.config.news.spin_min = 99.0;
    w.run_ticks(3 * TICKS_PER_DAY + 1);
    let st = news::feed_state(&w, civic).unwrap();
    assert!(!st.buried.iter().any(|&(a, _)| a == exec), "lapsed after bury_days: {:?}", st.buried);
}

#[test]
fn test_planted_story_moves_press_and_loyalty() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY);
    let vatra = corp_named(&w, "Vatra");
    let staff: Vec<EntityId> = ownership::employees_of(&w, vatra)
        .into_iter()
        .filter(|&e| demography::is_adult(&w, e) && w.comp::<Job>(e).is_some())
        .collect();
    assert!(staff.len() >= 5, "Vatra has staff");
    classes::compute(&mut w);
    let before: Vec<f32> = w.classes.iter().map(|c| c.loyalty).collect();
    let op0: f32 = staff.iter().map(|&e| reputation::opinion(&w, e, vatra)).sum::<f32>() / staff.len() as f32;
    // Each employee holds the planted story about their employer (slant −1).
    let now = w.tick;
    for &e in &staff {
        memory::hear_entry(&mut w, e, heard(vatra, Deed::Evicted, None, now, -100));
    }
    reputation::rebuild(&mut w);
    for &e in &staff {
        assert!(reputation::rep(&w, e).press < 0.0, "press below zero");
    }
    let op1: f32 = staff.iter().map(|&e| reputation::opinion(&w, e, vatra)).sum::<f32>() / staff.len() as f32;
    assert!(op1 < op0 - 0.05, "opinion of the employer falls: {op0:.3} -> {op1:.3}");
    // The press term on class loyalty, within press_cap of the M11 value.
    classes::compute(&mut w);
    let cap = w.config.news.press_cap;
    let mut moved = false;
    for (i, c) in w.classes.iter().enumerate() {
        let d = before[i] - c.loyalty;
        assert!(d <= cap + 1e-4, "class {i} loyalty fell {d} > cap {cap}");
        moved |= d > 0.0;
    }
    assert!(moved, "some class's loyalty fell: {before:?}");
}

#[test]
fn test_press_licence_off_only_civic_wire_publishes() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    w.levers.tax_rate = 0.0;
    apply(&mut w, PlayerCommand::SetPressLicence(false));
    assert!(!w.levers.press_licence);
    let ads = plain_adults(&w);
    let now = w.tick;
    for i in 0..4 {
        let e = entry(Deed::Assaulted, ads[2 * i], Some(ads[2 * i + 1]), now, 0);
        citysim::systems::gossip::post(&mut w, DistrictId(0), e);
    }
    let civic = civic_wire(&w);
    let nn = nutrix_now(&w);
    let t0 = w.treasury().map_or(0, |t| t.coins);
    news::daily(&mut w);
    assert!(!w.stories.is_empty());
    assert!(w.stories.iter().all(|s| s.feed == civic), "only the Civic Wire runs stories");
    assert!(stories_by(&w, nn).is_empty());
    // Every corp's ads go to the Civic Wire (the Treasury); Nutrix Now gets none.
    let n = w.corps().len() as i64;
    assert_eq!(w.stats.current.word.flow_ads, n * w.config.news.ad_rate);
    assert_eq!(w.treasury().map_or(0, |t| t.coins) - t0, n * w.config.news.ad_rate);
    assert_eq!(w.comp::<citysim::Building>(nn).map_or(0, |b| b.revenue_today), 0);
}

#[test]
fn test_censor_buries_faction_on_civic_wire() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    apply(&mut w, PlayerCommand::SetPressLicence(false));
    let g = w.gang_list()[0];
    let recruits: Vec<EntityId> =
        plain_adults(&w).into_iter().filter(|&a| w.comp::<Job>(a).is_none()).take(2).collect();
    for a in recruits {
        citysim::systems::gang::enlist(&mut w, a, g);
    }
    let members: Vec<EntityId> = w.comp::<citysim::Gang>(g).unwrap().members.clone();
    assert!(!members.is_empty());
    apply(&mut w, PlayerCommand::CensorStories { faction: g, on: true });
    assert!(w.levers.censored.contains(&g));
    let victim = plain_adults(&w)[0];
    let now = w.tick;
    citysim::systems::gossip::post(&mut w, DistrictId(0), entry(Deed::Killed, members[0], Some(victim), now, 0));
    news::daily(&mut w);
    let civic = civic_wire(&w);
    let about = |w: &World, s: &Story| s.actor == g || w.gang_of(s.actor) == Some(g);
    assert!(!stories_by(&w, civic).iter().any(|s| about(&w, s)), "nothing about the censored gang");
    // Lifted: the next day's paper runs it.
    apply(&mut w, PlayerCommand::CensorStories { faction: g, on: false });
    news::daily(&mut w);
    assert!(stories_by(&w, civic).iter().any(|s| about(&w, s)), "the gang's deed runs once lifted");
}

#[test]
fn test_ads_split_by_reach_and_civic_share_to_treasury() {
    let mut w = World::new(42, Config::load());
    clear_pools(&mut w);
    w.levers.tax_rate = 0.0;
    let civic = civic_wire(&w);
    let nn = nutrix_now(&w);
    let nutrix = corp_named(&w, "Nutrix");
    // Reach first (a day's refresh), then a clean day of ads.
    news::daily(&mut w);
    let (rc, rn) = (reach_of(&w, civic), reach_of(&w, nn));
    assert!(rc > 0.0 && rn > 0.0);
    let rate = w.config.news.ad_rate;
    // The largest-remainder split of `rate` by reach.
    let exact = [rate as f32 * rc / (rc + rn), rate as f32 * rn / (rc + rn)];
    let mut split = [exact[0].floor() as i64, exact[1].floor() as i64];
    let mut left = rate - split[0] - split[1];
    // Ties go to the lower Feed id.
    let ids = [civic, nn];
    let mut order = [0usize, 1];
    order.sort_by(|&a, &b| {
        (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())).then(ids[a].cmp(&ids[b]))
    });
    for i in order {
        if left > 0 {
            split[i] += 1;
            left -= 1;
        }
    }
    let n = w.corps().len() as i64;
    let t0 = w.treasury().map_or(0, |t| t.coins);
    let p0 = w.purse(Some(nutrix));
    let a0 = w.stats.current.word.flow_ads;
    news::daily(&mut w);
    assert_eq!(w.treasury().map_or(0, |t| t.coins) - t0, n * split[0], "the Civic Wire's share is the Treasury's");
    // Nutrix pays the Civic Wire its share and takes every other corp's Nutrix Now share.
    assert_eq!(w.purse(Some(nutrix)) - p0, (n - 1) * split[1] - split[0]);
    // A corp's own Feed's share stays home (no flow).
    assert_eq!(w.stats.current.word.flow_ads - a0, n * rate - split[1]);
}

#[test]
fn test_feeds_survive_save_and_load() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_DAY + 30);
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(news::all_feeds(&back).len(), 2);
    assert_eq!(back.stories.len(), w.stories.len());
    assert_eq!(citysim::save::to_ron(&back), text, "byte-identical round trip");
}

#[test]
fn test_plants_respect_burials_and_censorship() {
    let setup = || {
        let mut w = World::new(42, Config::load());
        clear_pools(&mut w);
        let nutrix = corp_named(&w, "Nutrix");
        let vatra = corp_named(&w, "Vatra");
        let rival_exec = w.comp::<Corp>(vatra).and_then(|c| c.exec).expect("Vatra's exec");
        let exec = w.comp::<Corp>(nutrix).and_then(|c| c.exec).expect("Nutrix's exec");
        let victim = plain_adults(&w).into_iter().find(|&v| v != exec && v != rival_exec).unwrap();
        let now = w.tick;
        memory::hear_entry(&mut w, exec, heard(rival_exec, Deed::Robbed, Some(victim), now, 0));
        w.config.news.spin_min = 0.0;
        (w, nutrix, vatra, rival_exec)
    };
    // The Civic Wire (the largest Feed Nutrix does not own) buries Vatra's exec: no plant on it.
    let (mut w, nutrix, _, rival_exec) = setup();
    let civic = civic_wire(&w);
    let until = w.tick + 2 * TICKS_PER_DAY;
    w.comp_mut::<citysim::Building>(civic).unwrap().feed.as_mut().unwrap().buried.push_back((rival_exec, until));
    news::daily(&mut w);
    assert_eq!(w.stats.current.word.planted, 0, "a buried actor is not planted");
    assert!(!w.stories.iter().any(|s| s.paid_by == Some(nutrix)));
    // The Civic Wire censors Vatra: the exec is Vatra's, so no plant either.
    let (mut w, nutrix, vatra, _) = setup();
    apply(&mut w, PlayerCommand::CensorStories { faction: vatra, on: true });
    news::daily(&mut w);
    assert_eq!(w.stats.current.word.planted, 0, "a censored faction is not planted on the Civic Wire");
    assert!(!w.stories.iter().any(|s| s.paid_by == Some(nutrix)));
    // Lifted, the plant runs.
    apply(&mut w, PlayerCommand::CensorStories { faction: vatra, on: false });
    news::daily(&mut w);
    assert!(w.stories.iter().any(|s| s.paid_by == Some(nutrix)), "the plant runs once lifted");
}
