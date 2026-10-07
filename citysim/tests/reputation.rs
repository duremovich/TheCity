//! M15 phase 1: the daily reputation rebuild and the faction matrix (plan
//! 1.12). Reputation is a set of numbers rebuilt from what fictional agents
//! hold in memory; these tests check the formulas and what they read.

use citysim::systems::{gossip, memory, reputation};
use citysim::word::{Axis, Deed};
use citysim::{Brain, Config, EntityId, Memory, MemoryEntry, MemoryKind, PlayerCommand, World, TICKS_PER_DAY};

fn word_world(seed: u64) -> World {
    let full = Config::load();
    let mut c = full.clone().v1_profile();
    c.gossip = full.gossip.clone();
    c.reputation = full.reputation.clone();
    c.moves.contradict_conf = full.moves.contradict_conf;
    World::new(seed, c)
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&id| w.has::<Brain>(id) && w.has::<Memory>(id) && citysim::systems::demography::is_adult(w, id))
        .collect()
}

#[test]
fn test_reputation_never_reads_life() {
    let mut w = word_world(61);
    let ads = adults(&w);
    let (killer, victim) = (ads[0], ads[1]);
    // A killing nobody saw and nobody holds: only the unnamed pool entry.
    w.kill_by(victim, citysim::DeathCause::Violence, Some(killer));
    for x in adults(&w) {
        let m = w.comp_mut::<Memory>(x).expect("mem");
        m.entries.retain(|e| e.subject != Some(killer));
        m.heard.retain(|e| e.subject != Some(killer));
    }
    let life_before = format!("{:?}", w.comp::<citysim::Life>(killer));
    reputation::rebuild(&mut w);
    let r = reputation::rep(&w, killer);
    assert_eq!(r.dread, 0.0, "an unwitnessed, unheld killing adds nothing: {r:?}");
    assert_eq!(life_before, format!("{:?}", w.comp::<citysim::Life>(killer)), "Life untouched");
    // Once someone holds it, it counts.
    let holder = adults(&w).into_iter().find(|&x| x != killer).expect("holder");
    let entry = MemoryEntry {
        subject: Some(killer),
        deed: Some(Deed::Killed),
        object: Some(victim),
        salience: 0.9,
        hops: 1,
        ..MemoryEntry::blank(MemoryKind::Rumour, w.tick)
    };
    memory::hear_entry(&mut w, holder, entry);
    reputation::rebuild(&mut w);
    assert!(reputation::rep(&w, killer).dread > 0.0);
}

#[test]
fn test_dread_saturates() {
    assert!((reputation::dread_of(3.0, 3.0) - (1.0 - (-1.0f32).exp())).abs() < 1e-6);
    assert!(reputation::dread_of(1e6, 3.0) <= 1.0);
    assert!(reputation::dread_of(30.0, 3.0) > 0.9999);
    assert_eq!(reputation::dread_of(0.0, 3.0), 0.0);
}

#[test]
fn test_reputation_numbers() {
    let cfg = Config::load().reputation;
    // Named killer, 25 holders: salience 0.5 after decay, conf 0.6, hops 2.
    let w1 = 0.5 * 0.6 * cfg.hop_w[2];
    assert!((w1 - 0.18).abs() < 1e-6);
    let d = 25.0 * w1 * cfg.dread_w.get(Deed::Killed);
    assert!((d - 4.5).abs() < 1e-4);
    assert!((reputation::dread_of(d, cfg.dread_scale) - 0.777).abs() < 1e-3);
    // Plus one pool entry: reach 0.5, pool_w 2, hops 1.
    let d2 = d + 0.5 * cfg.pool_w * cfg.hop_w[1];
    assert!((d2 - 5.3).abs() < 1e-4);
    assert!((reputation::dread_of(d2, cfg.dread_scale) - 0.829).abs() < 1e-3);
    // Two known robberies, w 0.3 each.
    let h = 2.0 * 0.3 * cfg.honour_w.get(Deed::Robbed);
    assert!((h + 0.18).abs() < 1e-6);
    assert!((reputation::honour_of(h, cfg.honour_scale) - 0.47).abs() < 1e-3);
    // Wanted, no deeds.
    assert!((reputation::heat_of(0.0, cfg.heat_scale, cfg.heat_wanted) - 0.6).abs() < 1e-6);
    // Corp exec known by 40.
    assert!((reputation::standing_of(0.9, 40, cfg.fame_scale) - 0.827).abs() < 1e-3);
    // Dreg known by 3, rags.
    assert!((reputation::standing_of(0.05, 3, cfg.fame_scale) - 0.024).abs() < 1e-3);
}

#[test]
fn test_pinned_reputation_holds_for_days() {
    let mut w = word_world(62);
    let who = adults(&w)[2];
    w.push_command(PlayerCommand::SetReputation { who, axis: Axis::Dread, value: 0.9, days: 3 });
    w.tick();
    assert!((reputation::rep(&w, who).dread - 0.9).abs() < 1e-6);
    // Two midnights later the pin holds; after the third it is gone.
    for day in 1..=2 {
        w.tick = day * TICKS_PER_DAY;
        reputation::rebuild(&mut w);
        assert!((reputation::rep(&w, who).dread - 0.9).abs() < 1e-6, "day {day}");
    }
    w.tick = 4 * TICKS_PER_DAY;
    reputation::rebuild(&mut w);
    let r = reputation::rep(&w, who);
    assert!(r.pinned.is_none());
    assert!(r.dread < 0.9, "the pin ran out: {}", r.dread);
}

#[test]
fn test_regard_fear_formula() {
    let mut w = word_world(63);
    let gangs = w.gang_list().to_vec();
    assert!(gangs.len() >= 2, "the v1 city has two gangs");
    let (a, b) = (gangs[0], gangs[1]);
    reputation::rebuild(&mut w);
    w.reputation[a.index as usize].as_mut().expect("gang rep").dread = 0.6;
    w.reputation[b.index as usize].as_mut().expect("gang rep").dread = 0.2;
    reputation::rebuild_regard(&mut w);
    assert!((reputation::regard(&w, b, a).fear - 0.9).abs() < 1e-6, "the weaker gang fears the stronger");
    assert!((reputation::regard(&w, a, b).fear - 0.1).abs() < 1e-6);
    // Every ordered pair of live factions has a cell; none to itself.
    let fs = reputation::factions(&w);
    assert_eq!(w.regard.len(), fs.len() * (fs.len() - 1));
}

#[test]
fn test_kill_watch_samples_known_by_seven_days_on() {
    let mut w = word_world(64);
    let ads = adults(&w);
    let killer = ads[0];
    gossip::watch_killer(&mut w, killer);
    let start = w.tick;
    for (i, &h) in ads.iter().skip(1).take(5).enumerate() {
        let e = MemoryEntry {
            subject: Some(killer),
            deed: Some(Deed::Killed),
            object: Some(ads[10 + i]),
            salience: 0.9,
            hops: 1,
            ..MemoryEntry::blank(MemoryKind::Rumour, start)
        };
        memory::hear_entry(&mut w, h, e);
    }
    w.tick = start + 7 * TICKS_PER_DAY;
    reputation::rebuild(&mut w);
    assert!(w.kill_known.last().is_some_and(|&k| k >= 5), "{:?}", w.kill_known);
    assert!(w.stats.current.word.known_by_killers_median >= 5.0);
}

#[test]
fn test_reused_index_inherits_no_reputation() {
    let mut w = word_world(65);
    let ads = adults(&w);
    let (dead, victim) = (ads[0], ads[1]);
    let holders: Vec<EntityId> = ads.iter().copied().skip(2).take(6).collect();
    for &h in &holders {
        let e = MemoryEntry {
            subject: Some(dead),
            deed: Some(Deed::Killed),
            object: Some(victim),
            salience: 0.9,
            hops: 1,
            ..MemoryEntry::blank(MemoryKind::Rumour, w.tick)
        };
        memory::hear_entry(&mut w, h, e);
    }
    reputation::rebuild(&mut w);
    assert!(reputation::rep(&w, dead).dread > 0.0 && reputation::known_by(&w, dead) >= 6);
    // The killer dies and leaves the world; an immigrant takes the index.
    w.kill(dead, citysim::DeathCause::Violence);
    w.remove_agent(dead);
    let fresh = citysim::systems::demography::spawn_immigrant(&mut w);
    assert_eq!(fresh.index, dead.index, "the freed index is reused");
    assert_ne!(fresh.generation, dead.generation);
    reputation::rebuild(&mut w);
    let r = reputation::rep(&w, fresh);
    assert_eq!(r.dread, 0.0, "the immigrant inherits no dread: {r:?}");
    assert_eq!(r.heat, 0.0);
    assert!((r.honour - 0.5).abs() < 1e-6);
    assert!(r.known_by < 6, "known_by not inherited: {}", r.known_by);
    assert_eq!(reputation::rep(&w, dead), citysim::word::Reputation::default(), "the stale id reads nothing");
    assert_eq!(reputation::known_by(&w, dead), 0);
}
