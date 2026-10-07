//! M15 phase 1: deeds, the heard store, the exchange, the pools, the kin
//! channel, distortion and the word streams (plan 1.12). Every "deed" is a
//! tagged record of an earlier event between fictional agents; these tests
//! check how such records are copied between agents.

use citysim::systems::{gossip, lod, memory};
use citysim::word::{Deed, DeedRef, PoolEntry, WordNs};
use citysim::{
    hole_id, Brain, Config, Crime, DistrictId, EntityId, Hole, HoleKind, Lod, Memory, MemoryEntry, MemoryKind, RelKind,
    World, Zone, TICKS_PER_DAY,
};
use rand::Rng;

/// The v1 city with the word on (v1_profile turns every M15 section off).
fn word_world(seed: u64) -> World {
    let full = Config::load();
    let mut c = full.clone().v1_profile();
    c.gossip = full.gossip.clone();
    c.reputation = full.reputation.clone();
    c.moves.contradict_conf = full.moves.contradict_conf;
    World::new(seed, c)
}

/// Adults with a Brain and a Memory, ascending.
fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&id| w.has::<Brain>(id) && w.has::<Memory>(id) && citysim::systems::demography::is_adult(w, id))
        .collect()
}

fn e(kind: MemoryKind, tick: u64) -> MemoryEntry {
    MemoryEntry::blank(kind, tick)
}

fn id(i: u32) -> EntityId {
    EntityId { index: i, generation: 0 }
}

#[test]
fn test_deed_of_reads_first_hand_kinds() {
    let holder = id(1);
    let (actor, victim, corp) = (id(2), id(3), id(4));
    let saw = |crime| MemoryEntry {
        subject: Some(actor),
        crime: Some(crime),
        object: Some(victim),
        salience: 0.8,
        ..e(MemoryKind::SawCrime, 10)
    };
    let want = |deed, a: Option<EntityId>, o: Option<EntityId>| Some(DeedRef { deed, actor: a, object: o });
    assert_eq!(memory::deed_of(holder, &saw(Crime::Murder)), want(Deed::Killed, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &saw(Crime::Assault)), want(Deed::Assaulted, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &saw(Crime::Theft)), want(Deed::Robbed, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &saw(Crime::GrandTheft)), want(Deed::Robbed, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &saw(Crime::Extortion)), want(Deed::Extorted, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &saw(Crime::Vagrancy)), None);
    assert_eq!(memory::deed_of(holder, &saw(Crime::Dealing)), None);
    let robbed = MemoryEntry { subject: Some(actor), ..e(MemoryKind::WasRobbed, 10) };
    assert_eq!(memory::deed_of(holder, &robbed), want(Deed::Robbed, Some(actor), Some(holder)));
    let lost = MemoryEntry { subject: Some(actor), ..e(MemoryKind::Lost, 10) };
    assert_eq!(memory::deed_of(holder, &lost), want(Deed::Assaulted, Some(actor), Some(holder)));
    let fought = MemoryEntry { subject: Some(actor), ..e(MemoryKind::Fought, 10) };
    assert_eq!(memory::deed_of(holder, &fought), None, "Fought alone is not a deed");
    let grief = MemoryEntry { subject: Some(victim), ..e(MemoryKind::Grief, 10) };
    assert_eq!(memory::deed_of(holder, &grief), want(Deed::Killed, None, Some(victim)));
    let evicted = MemoryEntry { subject: Some(corp), ..e(MemoryKind::Evicted, 10) };
    assert_eq!(memory::deed_of(holder, &evicted), want(Deed::Evicted, Some(corp), Some(holder)));
    let stripped = MemoryEntry { subject: Some(actor), object: Some(victim), ..e(MemoryKind::Stripped, 10) };
    assert_eq!(memory::deed_of(holder, &stripped), want(Deed::Stripped, Some(actor), Some(victim)));
    let rumour = MemoryEntry {
        subject: Some(actor),
        deed: Some(Deed::Married),
        object: Some(victim),
        hops: 2,
        ..e(MemoryKind::Rumour, 10)
    };
    assert_eq!(memory::deed_of(holder, &rumour), want(Deed::Married, Some(actor), Some(victim)));
    assert_eq!(memory::deed_of(holder, &e(MemoryKind::Socialised, 10)), None);
    assert_eq!(memory::deed_of(holder, &e(MemoryKind::Sighting, 10)), None);
}

#[test]
fn test_chat_passes_strongest_deed_with_hops_and_trust() {
    let mut w = word_world(51);
    let ads = adults(&w);
    // Speaker, listener, and two actors with no gang (a distortion roll has
    // nobody to swap to: the speaker has no enemies).
    let (a, b) = (ads[0], ads[1]);
    let free: Vec<EntityId> =
        ads.iter().copied().filter(|&x| x != a && x != b && w.gang_of(x).is_none()).take(3).collect();
    let (killer, robber, victim) = (free[0], free[1], free[2]);
    for x in [a, b] {
        w.comp_mut::<Memory>(x).expect("mem").entries.clear();
        w.comp_mut::<Memory>(x).expect("mem").heard.clear();
    }
    w.enemies.remove(&a);
    {
        let edge = w.edge_entry(a, b);
        edge.trust = 0.6;
    }
    let now = w.tick;
    let mem = w.comp_mut::<Memory>(a).expect("mem");
    mem.entries.push(MemoryEntry {
        subject: Some(killer),
        crime: Some(Crime::Murder),
        object: Some(victim),
        salience: 0.9,
        valence: -0.9,
        ..e(MemoryKind::SawCrime, now)
    });
    mem.entries.push(MemoryEntry {
        subject: Some(robber),
        salience: 0.5,
        valence: -0.5,
        ..e(MemoryKind::WasRobbed, now)
    });
    gossip::exchange(&mut w, a, b, gossip::Venue::Chat);
    let heard = &w.comp::<Memory>(b).expect("mem").heard;
    assert_eq!(heard.len(), 1, "one telling per exchange: {heard:?}");
    let r = &heard[0];
    assert_eq!(r.kind, MemoryKind::Rumour);
    assert_eq!(r.deed, Some(Deed::Killed), "the strongest deed goes first");
    assert_eq!(r.subject, Some(killer));
    assert_eq!(r.object, Some(victim));
    assert_eq!(r.hops, 1);
    assert!((r.conf - (0.5 + 0.5 * 0.6)).abs() < 1e-6, "conf {}", r.conf);
    assert!((r.salience - 0.9 * 0.7).abs() < 1e-6, "salience {}", r.salience);
    assert!(w.comp::<Memory>(b).expect("mem").entries.is_empty(), "talk never writes entries");
    // A duplicate merges: max conf, min hops.
    let mut m = Memory::default();
    let dup = |hops: u8, conf: f32| MemoryEntry {
        subject: Some(killer),
        deed: Some(Deed::Killed),
        object: Some(victim),
        salience: 0.5,
        hops,
        conf,
        ..e(MemoryKind::Rumour, now)
    };
    let h = b;
    assert_eq!(memory::insert_heard(&mut m, h, dup(3, 0.4), now, 8, 7.0, 0.3), memory::HeardInsert::Inserted);
    assert_eq!(memory::insert_heard(&mut m, h, dup(1, 0.9), now, 8, 7.0, 0.3), memory::HeardInsert::Merged);
    assert_eq!(memory::insert_heard(&mut m, h, dup(2, 0.5), now, 8, 7.0, 0.3), memory::HeardInsert::Merged);
    assert_eq!(m.heard.len(), 1);
    assert_eq!(m.heard[0].hops, 1);
    assert!((m.heard[0].conf - 0.9).abs() < 1e-6);
    // A different named actor for the same deed contradicts: both lose conf.
    let other = MemoryEntry { subject: Some(robber), ..dup(2, 0.8) };
    assert_eq!(memory::insert_heard(&mut m, h, other, now, 8, 7.0, 0.3), memory::HeardInsert::Contradicted);
    assert_eq!(m.heard.len(), 2);
    assert!((m.heard[0].conf - 0.6).abs() < 1e-6 && (m.heard[1].conf - 0.5).abs() < 1e-6, "{:?}", m.heard);
}

#[test]
fn test_anonymous_rumour_takes_name_when_hole_binds() {
    let mut w = word_world(52);
    w.config.bind.p_unknown = 0.0;
    // A day on, every adult has a trace for the binder's candidates.
    w.run_ticks(TICKS_PER_DAY + 1);
    let ads = adults(&w);
    let (victim, holder) = (ads[3], ads[4]);
    let tick = w.tick;
    let hole = Hole {
        id: hole_id(tick, victim, HoleKind::Robbed),
        kind: HoleKind::Robbed,
        victim,
        zone: Zone::ALL[0],
        district: DistrictId(0),
        tick,
        event_id: 0,
        consequential: false,
        spouse: None,
        loot: 0,
        home: None,
        gang: None,
    };
    let hid = citysim::systems::bind::open_hole(&mut w, hole);
    let pooled = w.rumours[0].entries.iter().find(|p| p.hole == Some(hid)).expect("posted at open_hole");
    assert_eq!(pooled.actor, None);
    assert_eq!(pooled.deed, Deed::Robbed);
    // Someone already heard of it unnamed.
    let anon = MemoryEntry {
        deed: Some(Deed::Robbed),
        object: Some(victim),
        salience: 0.5,
        hops: 1,
        ..e(MemoryKind::Rumour, tick)
    };
    memory::hear_entry(&mut w, holder, anon);
    let bound = citysim::systems::bind::bind(&mut w, hid).expect("open hole");
    let citysim::Bound::Actor(actor) = bound else { panic!("p_unknown 0 binds an actor") };
    let pooled = w.rumours[0].entries.iter().find(|p| p.hole == Some(hid)).expect("still pooled");
    assert_eq!(pooled.actor, Some(actor), "the pool entry takes the name");
    let held = w.comp::<Memory>(holder).expect("mem").heard.iter().find(|x| x.deed == Some(Deed::Robbed)).cloned();
    assert_eq!(held.and_then(|x| x.subject), Some(actor), "the held rumour takes the name");
}

#[test]
fn test_heard_never_evicts_first_hand() {
    let now = 5 * TICKS_PER_DAY;
    let mut m = Memory::default();
    for i in 0..24u32 {
        let x = MemoryEntry { subject: Some(id(100 + i)), salience: 0.06, ..e(MemoryKind::Socialised, now) };
        memory::insert(&mut m, x, now, 24, 7.0);
    }
    let rumour = |i: u32, sal: f32| MemoryEntry {
        subject: Some(id(200 + i)),
        deed: Some(Deed::Robbed),
        object: Some(id(300 + i)),
        salience: sal,
        hops: 2,
        ..e(MemoryKind::Rumour, now)
    };
    for i in 0..8 {
        let out = memory::insert_heard(&mut m, id(1), rumour(i, 0.5 + 0.01 * i as f32), now, 8, 7.0, 0.3);
        assert_eq!(out, memory::HeardInsert::Inserted);
    }
    assert_eq!((m.entries.len(), m.heard.len()), (24, 8));
    // A 9th, stronger than the weakest heard: the weakest heard goes, never a first-hand entry.
    assert_eq!(memory::insert_heard(&mut m, id(1), rumour(8, 0.9), now, 8, 7.0, 0.3), memory::HeardInsert::Inserted);
    assert_eq!((m.entries.len(), m.heard.len()), (24, 8));
    assert!(!m.heard.iter().any(|x| x.subject == Some(id(200))), "the weakest heard was evicted");
    // A 10th weaker than every heard entry is dropped.
    assert_eq!(memory::insert_heard(&mut m, id(1), rumour(9, 0.1), now, 8, 7.0, 0.3), memory::HeardInsert::Dropped);
    // Off screen: 8 first-hand and 3 heard; no Sighting.
    let mut w = word_world(53);
    let x = adults(&w)[5];
    lod::set_lod(&mut w, x, Lod::Statistical);
    assert_eq!(w.comp::<Brain>(x).map(|b| b.lod), Some(Lod::Statistical));
    {
        let mm = w.comp_mut::<Memory>(x).expect("mem");
        mm.heard.clear();
        mm.entries.clear();
    }
    for i in 0..8 {
        w.remember(x, MemoryKind::WasRobbed, Some(id(400 + i)), 0.6, -0.6, false);
    }
    for i in 0..5 {
        memory::hear_entry(&mut w, x, rumour(10 + i, 0.5 + 0.01 * i as f32));
    }
    let mm = w.comp::<Memory>(x).expect("mem");
    assert_eq!((mm.entries.len(), mm.heard.len()), (8, 3));
    let seen = MemoryEntry { subject: Some(id(500)), salience: 0.5, ..e(MemoryKind::Sighting, w.tick) };
    assert_eq!(memory::hear_entry(&mut w, x, seen), memory::HeardInsert::Dropped, "no eyes off screen");
}

#[test]
fn test_entries_unchanged_by_heard() {
    let now = 3 * TICKS_PER_DAY;
    let (mut quiet, mut busy) = (Memory::default(), Memory::default());
    for i in 0..40u32 {
        let x = MemoryEntry {
            subject: Some(id(i % 13)),
            salience: 0.2 + 0.02 * (i % 17) as f32,
            ..e(if i % 3 == 0 { MemoryKind::WasRobbed } else { MemoryKind::Socialised }, now + u64::from(i) * 7)
        };
        memory::insert(&mut quiet, x.clone(), now + u64::from(i) * 7, 24, 7.0);
        memory::insert(&mut busy, x, now + u64::from(i) * 7, 24, 7.0);
        let r = MemoryEntry {
            subject: Some(id(1000 + i)),
            deed: Some(Deed::Assaulted),
            object: Some(id(2000 + i)),
            salience: 0.9,
            hops: 1,
            ..e(MemoryKind::Rumour, now + u64::from(i) * 7)
        };
        memory::insert_heard(&mut busy, id(7), r, now + u64::from(i) * 7, 8, 7.0, 0.3);
    }
    assert!(!busy.heard.is_empty());
    assert_eq!(format!("{:?}", quiet.entries), format!("{:?}", busy.entries), "same entries, same order");
}

#[test]
fn test_pool_decays_leaks_adjacent_only_and_drops() {
    let mut w = World::new(54, Config::load());
    for p in w.rumours.iter_mut() {
        p.entries.clear();
    }
    let d = 3usize; // Mid West
    let adj = w.district_adjacent[d];
    assert!(adj != 0, "Mid West has neighbours");
    let now = w.tick;
    let entry = |object: u32, reach: f32| PoolEntry {
        deed: Deed::Assaulted,
        actor: Some(id(9)),
        object: Some(id(object)),
        tick: now,
        hops: 0,
        reach,
        hole: None,
        story: None,
        kin: Default::default(),
        told: Default::default(),
        district: DistrictId(d as u8),
        press: 0,
    };
    let (big, small) = (entry(10, 1.0), entry(11, 0.06));
    gossip::post(&mut w, DistrictId(d as u8), big);
    gossip::post(&mut w, DistrictId(d as u8), small);
    gossip::decay_and_leak(&mut w);
    let here = &w.rumours[d].entries;
    assert_eq!(here.len(), 1, "the 0.06 entry fell below reach_min: {here:?}");
    assert!((here[0].reach - 0.8).abs() < 1e-6);
    for n in 0..w.rumours.len() {
        if n == d {
            continue;
        }
        let got = &w.rumours[n].entries;
        if adj & (1 << n) != 0 {
            assert_eq!(got.len(), 1, "district {n} is adjacent");
            assert!((got[0].reach - 0.4).abs() < 1e-6, "0.4 x 1.0, got {}", got[0].reach);
            assert_eq!(got[0].hops, 1);
            assert!(got[0].kin.is_empty());
        } else {
            assert!(got.is_empty(), "district {n} is not adjacent");
        }
    }
    // The cap: 64, the lowest reach out.
    for p in w.rumours.iter_mut() {
        p.entries.clear();
    }
    for i in 0..65u32 {
        gossip::post(&mut w, DistrictId(0), entry(100 + i, 0.1 + 0.01 * i as f32));
    }
    let pool = &w.rumours[0].entries;
    assert_eq!(pool.len(), 64);
    assert!(!pool.iter().any(|p| p.object == Some(id(100))), "the lowest reach went");
}

#[test]
fn test_kin_channel_reaches_friend_in_other_district() {
    let mut w = World::new(55, Config::load());
    let (spire, east) = (DistrictId(0), DistrictId(7));
    assert_eq!(w.district_name(spire), "Spire");
    assert_eq!(w.district_name(east), "Sump East");
    let ads = adults(&w);
    let lives = |w: &World, x: EntityId, d: DistrictId| gossip::home_district(w, x) == Some(d);
    let victim = ads.iter().copied().find(|&x| lives(&w, x, east)).expect("a Sump East adult");
    let friend = ads.iter().copied().find(|&x| lives(&w, x, spire) && w.edge(victim, x).is_none()).expect("Spire");
    let killer = ads.iter().copied().find(|&x| x != victim && x != friend && w.gang_of(x).is_none()).expect("killer");
    {
        let edge = w.edge_entry(victim, friend);
        edge.kind = RelKind::Friend;
        edge.affinity = 0.7;
    }
    lod::set_lod(&mut w, friend, Lod::Statistical);
    w.kill_by(victim, citysim::DeathCause::Violence, Some(killer));
    let posted: Vec<&PoolEntry> =
        w.rumours.iter().flat_map(|p| p.entries.iter()).filter(|p| p.object == Some(victim)).collect();
    assert_eq!(posted.len(), 1, "one anonymous killing in the death's pool");
    assert!(posted[0].kin.contains(&friend), "the friend captured before the unlink");
    assert_eq!(posted[0].actor, None);
    gossip::name_actor(&mut w, victim, killer);
    let r = DeedRef { deed: Deed::Killed, actor: Some(killer), object: Some(victim) };
    let death = w.tick;
    let mut got = None;
    for day in 1..=3 {
        w.tick = death + day * TICKS_PER_DAY;
        gossip::kin(&mut w);
        if memory::holds_deed(friend, w.comp::<Memory>(friend).expect("mem"), &r, death) {
            got = Some(day);
            break;
        }
    }
    assert!(got.is_some(), "the Statistical friend in the Spire heard within 3 days");
    assert_eq!(w.comp::<Brain>(friend).map(|b| b.lod), Some(Lod::Statistical));
}

#[test]
fn test_distortion_never_at_knowledge_one() {
    let mut w = World::new(56, Config::load());
    let gang = w.gang_list()[0];
    let ads = adults(&w);
    let (member, speaker) = (ads[0], ads[1]);
    w.insert(member, citysim::GangMember { gang, rank: 0, joined_tick: 0 });
    let mut swaps = [0u32; 2];
    for (k, knowledge) in [1.0f32, 0.0].into_iter().enumerate() {
        for i in 0..2000u64 {
            let mut rng = w.rng.word(WordNs::Distort, i, 1);
            let mut r = DeedRef { deed: Deed::Killed, actor: Some(member), object: None };
            if gossip::distort_with(&w, speaker, &mut r, DistrictId(0), &mut rng, knowledge) {
                swaps[k] += 1;
                assert_eq!(r.actor, Some(gang), "a swap names the actor's gang");
            }
        }
    }
    assert_eq!(swaps[0], 0, "knowledge 1 never distorts");
    assert!(swaps[1] > 200, "knowledge 0 distorts at distort_base: {}", swaps[1]);
}

#[test]
fn test_word_streams_untouch_world_and_agent() {
    let run = |on: bool| {
        let mut w = word_world(57);
        if !on {
            w.config.gossip.enabled = false;
        }
        // L1: four days (was two): with the walk to a far Bar weighed, the
        // first drinks and chats come later and the 2-day window heard nothing.
        w.run_ticks(4 * TICKS_PER_DAY + 5);
        let heard = w.stats.history.iter().map(|r| r.word.rumours_heard).sum::<u32>();
        let probe = adults(&w)[0];
        let world_next: u64 = w.rng.world().random();
        let agent_next: u64 = w.rng.agent(probe).random();
        (heard, world_next, agent_next)
    };
    let (heard_on, w_on, a_on) = run(true);
    let (heard_off, w_off, a_off) = run(false);
    assert!(heard_on > 0, "the day had exchanges and hearing");
    assert_eq!(heard_off, 0);
    assert_eq!(w_on, w_off, "the world stream is untouched");
    assert_eq!(a_on, a_off, "the agent stream is untouched");
    // And a word stream is its own: keyed, repeatable, apart from the world's.
    let w = word_world(57);
    let a: u64 = w.rng.word(WordNs::Exchange, 5, 6).random();
    let b: u64 = w.rng.word(WordNs::Exchange, 5, 6).random();
    let c: u64 = w.rng.word(WordNs::Hear, 5, 6).random();
    assert_eq!(a, b);
    assert_ne!(a, c);
}
