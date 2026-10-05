//! M10 phase 3: off-screen outcomes, holes and the binder.

use citysim::systems::{bind, gang, lod};
use citysim::{
    hole_id, save, trace_flags, Bound, Building, Config, Corpse, DeathCause, EntityId, EventKind, Gang, Hole, HoleId,
    HoleKind, Household, Job, Lod, Personality, Position, Shock, StatRow, StatTable, Tick, Trace, World, STAT_ROWS,
    TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn table(f: impl Fn(&mut StatRow)) -> StatTable {
    let mut rows = vec![StatRow::default(); STAT_ROWS];
    for (i, r) in rows.iter_mut().enumerate() {
        r.label = StatTable::label(i);
        f(r);
    }
    StatTable {
        version: 2,
        seed: 0,
        days: 0,
        agents: 0,
        lawfulness_edges: [0.3, 0.7],
        hunger_edge: 0.4,
        p_dole_day: 1.0,
        rows,
    }
}

/// A v1 world, every agent forced Statistical, on an all-zero table.
fn stat_world(seed: u64) -> World {
    let mut cfg = Config::load().v1_profile();
    cfg.lod.force = Some(Lod::Statistical);
    let mut w = World::new(seed, cfg);
    w.stat_table = Some(table(|_| {}));
    w
}

/// A v1 world on normal tiers and an all-zero table, run past day 0 so
/// every adult has a trace for it.
fn traced_world(seed: u64) -> World {
    let mut w = World::new(seed, Config::load().v1_profile());
    w.stat_table = Some(table(|_| {}));
    w.run_ticks(TICKS_PER_DAY + 1);
    w
}

/// Open a hole by hand on `victim` at `tick`.
fn manual_hole(w: &mut World, victim: EntityId, kind: HoleKind, tick: Tick) -> HoleId {
    let tile = w.comp::<Position>(victim).expect("pos").tile;
    let zone = w.map.zone(tile);
    let ev = w.push_event(EventKind::Assaulted, &[EntityId::NONE, victim], "test hole");
    let hole = Hole {
        id: hole_id(tick, victim, kind),
        kind,
        victim,
        zone,
        tick,
        event_id: ev,
        consequential: false,
        spouse: w.spouse_of(victim),
        loot: 0,
        home: w.comp::<Household>(victim).and_then(|h| h.home),
        gang: w.gang_of(victim),
    };
    bind::open_hole(w, hole)
}

/// Mark `id`'s trace for `day` with `flag`.
fn set_trace_flag(w: &mut World, id: EntityId, day: u64, flag: u8) {
    let t = w.comp_mut::<Trace>(id).expect("trace");
    let back = (t.last_day - day) as usize;
    let n = t.days.len();
    t.days[n - 1 - back].flags |= flag;
}

fn adults_with_trace(w: &World) -> Vec<EntityId> {
    w.with::<Trace>().into_iter().filter(|&id| citysim::systems::law::living(w, id)).collect()
}

fn attributed_texts(w: &World) -> Vec<String> {
    w.events.iter().filter(|e| e.kind == EventKind::Attributed).map(|e| e.text.clone()).collect()
}

#[test]
fn test_p_killed_kills_with_violence_and_opens_consequential_hole() {
    let mut w = stat_world(301);
    w.stat_table = Some(table(|r| r.p_killed = 1.0));
    w.tick(); // tick 0: the slot-0 agents run
    let holes: Vec<Hole> = w.holes.values().cloned().collect();
    assert!(!holes.is_empty(), "no hole opened");
    for h in &holes {
        assert_eq!(h.kind, HoleKind::Killed);
        assert!(h.consequential);
        assert_eq!(h.victim.index % 60, 0, "only the slot-0 agents ran");
        assert_eq!(w.comp::<Corpse>(h.victim).map(|c| c.cause), Some(DeathCause::Violence));
        let ev = w.events.iter().find(|e| e.id == h.event_id).expect("event");
        assert_eq!(ev.kind, EventKind::Murder);
        assert_eq!(ev.actors[0], EntityId::NONE);
        assert_eq!(ev.actors[1], h.victim);
    }
    assert_eq!(w.stats.current.deaths_violence_offscreen as usize, holes.len());
    assert_eq!(w.stats.current.holes_opened as usize, holes.len());
}

#[test]
fn test_daily_pass_binds_consequential_hole() {
    let mut w = stat_world(302);
    w.stat_table = Some(table(|r| r.p_killed = 1.0));
    w.tick();
    let n = w.holes.len();
    assert!(n > 0);
    w.stat_table = Some(table(|_| {}));
    while w.tick_of_day() != 0 || w.tick < TICKS_PER_DAY {
        w.tick();
    }
    w.tick(); // the daily pass runs at tick_of_day 0
    assert!(w.holes.values().all(|h| h.kind != HoleKind::Killed), "a Killed hole survived the daily pass");
    assert_eq!(attributed_texts(&w).len(), n, "one Attributed event per hole");
    let bound = w.stats.history.iter().map(|r| r.holes_bound + r.holes_unknown).sum::<u32>()
        + w.stats.current.holes_bound
        + w.stats.current.holes_unknown;
    assert_eq!(bound as usize, n);
}

#[test]
fn test_binding_identical_across_save_load() {
    let mut w = traced_world(303);
    let victims: Vec<EntityId> = w.tier(Lod::Statistical).iter().copied().take(6).collect();
    let ids: Vec<HoleId> = victims.iter().map(|&v| manual_hole(&mut w, v, HoleKind::Robbed, 100)).collect();
    let mut loaded = save::from_ron(&save::to_ron(&w)).expect("round trip");
    for id in ids {
        let a = bind::bind(&mut w, id);
        let b = bind::bind(&mut loaded, id);
        assert_eq!(a, b, "hole {id}");
        assert!(a.is_some());
    }
    assert_eq!(attributed_texts(&w), attributed_texts(&loaded), "same actors, witnesses and texts");
}

#[test]
fn test_binding_identical_across_call_order() {
    let mut w = traced_world(304);
    let stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let (va, vb) = (stat[0], stat[stat.len() / 2]);
    let a = manual_hole(&mut w, va, HoleKind::Assaulted, 200);
    let b = manual_hole(&mut w, vb, HoleKind::Robbed, 300);
    let mut w2 = w.clone();
    let (a1, b1) = (bind::bind(&mut w, a), bind::bind(&mut w, b));
    let (b2, a2) = (bind::bind(&mut w2, b), bind::bind(&mut w2, a));
    assert_eq!(a1, a2);
    assert_eq!(b1, b2);
    let mut t1 = attributed_texts(&w);
    let mut t2 = attributed_texts(&w2);
    t1.sort();
    t2.sort();
    assert_eq!(t1, t2);
}

#[test]
fn test_enemy_in_zone_gets_documented_weight() {
    let mut w = traced_world(305);
    let stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let victim = stat[0];
    let id = manual_hole(&mut w, victim, HoleKind::Assaulted, 300);
    let hole = w.holes[&id].clone();
    let before = bind::candidates(&w, &hole);
    let (enemy, w_before) = *before.iter().find(|&&(c, _)| c != victim && w.spouse_of(victim) != Some(c)).expect("c");
    citysim::systems::social::make_enemy(&mut w, victim, enemy, -0.6);
    let after = bind::candidates(&w, &hole);
    let w_after = after.iter().find(|&&(c, _)| c == enemy).map(|&(_, x)| x).expect("still a candidate");
    // (1 - l)^2 x (1 + 3 x enemy) x ...: the enemy weighs four times as much.
    assert!((w_after / w_before - 4.0).abs() < 1e-9, "{w_before} -> {w_after}");
    // And the base term is (1 - l)^2 times the documented multipliers.
    let l = f64::from(w.comp::<Personality>(enemy).expect("p").lawfulness);
    let t = w.comp::<Trace>(enemy).and_then(|t| t.on_day(0)).expect("trace day 0");
    let s = if t.has(trace_flags::STATISTICAL_ALL_DAY) { 1.5 } else { 1.0 };
    let z = if t.zone == hole.zone { 1.0 } else { 0.25 };
    let g = if t.has(trace_flags::GANG)
        && hole.home.and_then(|h| w.comp::<Building>(h)).and_then(|b| b.claim).map(|c| c.gang) == w.gang_of(enemy)
    {
        3.0
    } else {
        1.0
    };
    let expected = (1.0 - l).powi(2) * 4.0 * s * z * g;
    assert!((w_after - expected).abs() < 1e-9, "{w_after} vs {expected}");
    // Every other candidate is unchanged.
    for (&(c0, x0), &(c1, x1)) in before.iter().zip(&after) {
        assert_eq!(c0, c1);
        if c0 != enemy {
            assert_eq!(x0, x1);
        }
    }
}

#[test]
fn test_no_candidate_binds_unknown() {
    let mut w = traced_world(306);
    w.config.bind.p_unknown = 0.0;
    let victim = w.tier(Lod::Statistical)[0];
    let id = manual_hole(&mut w, victim, HoleKind::Robbed, 400);
    for o in adults_with_trace(&w) {
        if o != victim {
            set_trace_flag(&mut w, o, 0, trace_flags::JAILED);
        }
    }
    assert!(bind::candidates(&w, &w.holes[&id].clone()).is_empty());
    assert_eq!(bind::bind(&mut w, id), Some(Bound::Unknown));
    assert!(attributed_texts(&w).last().is_some_and(|t| t.contains("never be solved")));
    let ev = w.events.iter().find(|e| e.text.starts_with("test hole")).expect("hole event");
    assert_eq!(ev.actors[0], EntityId::NONE);
    assert!(ev.text.ends_with("(never named)"));
}

#[test]
fn test_bound_gang_member_victim_shocks_gang() {
    let mut w = traced_world(307);
    w.config.bind.p_unknown = 0.0;
    let gangs = w.gangs();
    assert!(gangs.len() >= 2, "the v1 map has two Hideouts");
    let (ours, theirs) = (gangs[0], gangs[1]);
    let stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let (victim, rival) = (stat[0], stat[1]);
    gang::enlist(&mut w, rival, theirs);
    // Only the rival is free that day.
    for o in adults_with_trace(&w) {
        if o != victim && o != rival {
            set_trace_flag(&mut w, o, 0, trace_flags::JAILED);
        }
    }
    let id = manual_hole(&mut w, victim, HoleKind::Killed, 500);
    // The victim was one of ours when it happened.
    w.holes.get_mut(&id).expect("hole").gang = Some(ours);
    if let Some(g) = w.comp_mut::<Gang>(ours) {
        g.shocks.clear();
    }
    assert_eq!(bind::bind(&mut w, id), Some(Bound::Actor(rival)));
    let shocks = w.comp::<Gang>(ours).map(|g| g.shocks.clone()).unwrap_or_default();
    assert!(shocks.contains(&Shock::MemberKilled { by_rival: true }), "{shocks:?}");
}

#[test]
fn test_bound_actor_with_witness_is_reported_and_arrested() {
    let mut w = traced_world(308);
    w.config.bind.p_unknown = 0.0;
    w.config.bind.p_witness = 10.0; // x coverage >= 0.5: always witnessed
    let stat: Vec<EntityId> = w.tier(Lod::Statistical).to_vec();
    let victim = stat[0];
    let actor = *stat[1..]
        .iter()
        .find(|&&id| !w.has::<Job>(id) && w.spouse_of(victim) != Some(id))
        .expect("a jobless statistical agent");
    for o in adults_with_trace(&w) {
        if o != victim && o != actor {
            set_trace_flag(&mut w, o, 0, trace_flags::JAILED);
        }
    }
    let id = manual_hole(&mut w, victim, HoleKind::Assaulted, 600);
    assert_eq!(bind::bind(&mut w, id), Some(Bound::Actor(actor)));
    assert!(w.crime_reports.iter().any(|r| r.suspect == actor && r.witness.is_some() && !r.resolved));
    assert!(w.comp::<citysim::Brain>(actor).is_some_and(|b| b.lod != Lod::Statistical), "the actor got a body");
    let mut arrested = false;
    let mut cursor = w.next_event_id;
    for _ in 0..(10 * TICKS_PER_DAY) {
        w.tick();
        // Arrest events are [guard, suspect].
        arrested = w
            .events
            .iter()
            .filter(|e| e.id >= cursor)
            .any(|e| e.kind == EventKind::Arrest && e.actors.get(1) == Some(&actor));
        cursor = w.next_event_id;
        if arrested {
            break;
        }
    }
    assert!(arrested, "the cold case never led to an arrest");
}

#[test]
fn test_per_agent_cap_expires_oldest() {
    let mut w = traced_world(309);
    let victim = w.tier(Lod::Statistical)[0];
    let cap = w.config.bind.max_open_per_agent;
    let ids: Vec<HoleId> =
        (0..=cap as u64).map(|k| manual_hole(&mut w, victim, HoleKind::Robbed, 10 + k * TICKS_PER_HOUR)).collect();
    let open = w.holes_by_agent.get(&victim).cloned().unwrap_or_default();
    assert_eq!(open.len(), cap);
    assert!(!w.holes.contains_key(&ids[0]), "the oldest expired");
    assert!(open.iter().all(|h| ids[1..].contains(h)));
    assert_eq!(w.stats.current.holes_unknown, 1);
    assert!(attributed_texts(&w).last().is_some_and(|t| t.contains("never be solved")));
}

#[test]
fn test_promotion_binds_open_holes() {
    let mut w = traced_world(310);
    let victim = w.tier(Lod::Statistical)[0];
    manual_hole(&mut w, victim, HoleKind::Robbed, 700);
    manual_hole(&mut w, victim, HoleKind::Assaulted, 800);
    assert_eq!(w.holes_by_agent.get(&victim).map(|l| l.len()), Some(2));
    lod::set_lod(&mut w, victim, Lod::Coarse);
    w.tick();
    assert!(!w.holes_by_agent.contains_key(&victim), "holes still open after promotion");
    assert!(w.holes.values().all(|h| h.victim != victim));
}

#[test]
fn test_trace_written_daily_and_survives_death() {
    let mut w = traced_world(311);
    let id = w.tier(Lod::Statistical)[0];
    let t = w.comp::<Trace>(id).cloned().expect("a trace after day 0");
    assert_eq!(t.last_day, 0);
    let d0 = t.on_day(0).expect("day 0");
    assert!(d0.has(trace_flags::ALIVE));
    // Round trip through the packed u32.
    let packed: u32 = d0.into();
    assert_eq!(citysim::DayTrace::from(packed), d0);
    w.kill(id, DeathCause::OldAge);
    assert!(w.has::<Trace>(id), "the dead keep their past");
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(w.comp::<Trace>(id).map(|t| t.last_day), Some(0), "no entries after death");
}
