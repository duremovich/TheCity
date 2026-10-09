//! M15 phase 3 (plan 3.8): grudges, their inheritance and settlement, the
//! Hunt (eligibility, intel, stake-out promotion, the strike), street
//! silence, vendettas and Retaliate, the fear term, honour-weighted
//! contracts and Guard the body. Every grudge, hunt and strike here is
//! game state between fictional agents: a utility score, a scripted plan
//! of abstract actions and one seeded dice roll.

use citysim::systems::{demography, faction, grudges, hunt, law, memory, raid};
use citysim::word::{Deed, GrudgeCause, Grudges, HuntPhase, HuntWhy, IntelSource, Reputation};
use citysim::{
    ActionKind, Brain, BuildingKind, Config, Corp, DeathCause, EntityId, ExecState, GoalKind, Identity, Job, Lod,
    MemoryEntry, MemoryKind, Order, Plan, PlayerCommand, Position, RelKind, Skills, Trace, World, TICKS_PER_DAY,
    TICKS_PER_HOUR,
};

/// The first gang, with six jobless adults enlisted (the first leads).
fn led_gang(w: &mut World) -> EntityId {
    let g = w.gang_list()[0];
    let recruits: Vec<EntityId> =
        adults(w).into_iter().filter(|&a| !w.has::<Job>(a) && w.gang_of(a).is_none()).take(6).collect();
    for a in recruits {
        citysim::systems::gang::enlist(w, a, g);
    }
    assert!(w.comp::<citysim::Gang>(g).and_then(|x| x.leader).is_some(), "a leader");
    g
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

fn grudge(w: &World, holder: EntityId, target: EntityId) -> Option<citysim::word::Grudge> {
    w.comp::<Grudges>(holder).and_then(|g| g.list.iter().find(|x| x.target == target).cloned())
}

fn heard_killing(w: &mut World, holder: EntityId, killer: EntityId, victim: EntityId, conf: f32) {
    let e = MemoryEntry {
        subject: Some(killer),
        salience: 0.9,
        valence: -0.9,
        second_hand: true,
        deed: Some(Deed::Killed),
        object: Some(victim),
        hops: 2,
        conf,
        ..MemoryEntry::blank(MemoryKind::Rumour, w.tick)
    };
    memory::hear_entry(w, holder, e);
}

/// Three adults unrelated to each other (no edge between any two).
fn three(w: &World) -> (EntityId, EntityId, EntityId) {
    let ads = adults(w);
    for &a in &ads {
        for &b in &ads {
            if a == b || w.edge(a, b).is_some() {
                continue;
            }
            for &c in &ads {
                if c != a && c != b && w.edge(a, c).is_none() && w.edge(b, c).is_none() {
                    return (a, b, c);
                }
            }
        }
    }
    panic!("three strangers");
}

#[test]
fn test_grudge_forms_at_spouse_weight_on_heard_killing() {
    let mut w = World::new(42, Config::load());
    let (a, b, k) = three(&w);
    w.set_spouse(a, b);
    heard_killing(&mut w, a, k, b, 0.6);
    let g = grudge(&w, a, k).expect("a grudge on the killer");
    assert!((g.weight - 0.6).abs() < 1e-5, "sev 1 x spouse 1 x conf 0.6: {}", g.weight);
    assert_eq!(g.cause, GrudgeCause::KilledKin(b));
    assert_eq!(w.edge(a, k).map(|e| e.kind), Some(RelKind::Enemy), "the killer is an Enemy");
    // A stranger's killing leaves nothing.
    let (x, y, z) = three(&w);
    heard_killing(&mut w, x, z, y, 1.0);
    assert!(grudge(&w, x, z).is_none(), "no tie, no grudge");
}

#[test]
fn test_grudge_inherits_on_holder_death() {
    let mut w = World::new(42, Config::load());
    let ads = adults(&w);
    let (h, t, _) = three(&w);
    let born = |w: &World, x: EntityId| w.comp::<Identity>(x).map_or(0, |i| i.born_tick);
    let s =
        ads.iter().copied().find(|&x| x != h && x != t && w.edge(x, h).is_none() && w.edge(x, t).is_none()).unwrap();
    let c = ads
        .iter()
        .copied()
        .find(|&x| x != h && x != t && x != s && born(&w, x) > born(&w, h) && w.edge(x, h).is_none())
        .expect("a younger adult");
    w.set_spouse(h, s);
    w.edge_entry(c, h).kind = RelKind::Parent;
    grudges::add(&mut w, h, t, GrudgeCause::Assaulted, 0.8, 2);
    w.kill_by(h, DeathCause::Violence, None);
    for heir in [s, c] {
        let g = grudge(&w, heir, t).expect("inherited");
        assert!((g.weight - 0.48).abs() < 1e-5, "x inherit_frac 0.6: {}", g.weight);
        assert_eq!(g.cause, GrudgeCause::Inherited(h));
        assert_eq!(g.chain, 2, "chain kept");
    }
    assert!(w.stats.current.word.grudges_inherited >= 2);
}

#[test]
fn test_grudge_settles_on_target_death_by_anyone() {
    let mut w = World::new(42, Config::load());
    let (h, t, other) = three(&w);
    grudges::add(&mut w, h, t, GrudgeCause::Robbed, 0.7, 0);
    assert!(grudges::holds(&w, h, t, 0.5));
    w.kill_by(t, DeathCause::Violence, Some(other));
    let g = grudge(&w, h, t).expect("kept for the biography");
    assert!(g.settled.is_some(), "settled by anyone's hand");
    assert!(!grudges::holds(&w, h, t, 0.0));
}

#[test]
fn test_hunt_gates_on_max_hunts() {
    let mut cfg = Config::load();
    cfg.hunt.max_hunts = 1;
    let mut w = World::new(42, cfg);
    let ads = adults(&w);
    let (h1, t1, h2) = three(&w);
    let t2 = ads.iter().copied().find(|&x| ![h1, t1, h2].contains(&x)).unwrap();
    grudges::add(&mut w, h1, t1, GrudgeCause::Assaulted, 1.0, 0);
    grudges::add(&mut w, h2, t2, GrudgeCause::Assaulted, 1.0, 0);
    assert!(hunt::considerations(&w, h1).is_some(), "eligible");
    assert!(hunt::considerations(&w, h2).is_some(), "eligible");
    assert!(hunt::adopt(&mut w, h1, HuntWhy::Goal));
    assert!(hunt::considerations(&w, h2).is_none(), "the cap is full");
    assert!(!hunt::adopt(&mut w, h2, HuntWhy::Goal));
    assert!(hunt::considerations(&w, h1).is_some(), "a hunter is scored whatever the cap");
    // Below hunt_min nobody hunts.
    let mut w2 = World::new(42, Config::load());
    grudges::add(&mut w2, h1, t1, GrudgeCause::Assaulted, 0.3, 0);
    assert!(hunt::considerations(&w2, h1).is_none());
}

#[test]
fn test_hunted_statistical_target_promoted_at_stakeout() {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_HOUR);
    let lod = |w: &World, x: EntityId| w.comp::<Brain>(x).map(|b| b.lod);
    let stat: Vec<EntityId> = adults(&w).into_iter().filter(|&x| lod(&w, x) == Some(Lod::Statistical)).collect();
    let body = adults(&w).into_iter().find(|&x| lod(&w, x) != Some(Lod::Statistical)).expect("a body");
    let t = stat.into_iter().find(|&x| w.edge(x, body).is_none()).expect("a Statistical stranger");
    grudges::add(&mut w, body, t, GrudgeCause::Assaulted, 1.0, 0);
    assert!(hunt::adopt(&mut w, body, HuntWhy::Goal));
    // Asking: the target is not promoted yet.
    hunt::on_goto_intel(&mut w, body);
    assert_eq!(lod(&w, t), Some(Lod::Statistical));
    let intel = hunt::habit(&w, t, w.tick);
    if let Some(s) = w.hunts.get_mut(&body) {
        s.phase = HuntPhase::Watch;
        s.intel = Some(intel);
    }
    hunt::on_goto_intel(&mut w, body);
    assert_eq!(lod(&w, t), Some(Lod::Coarse), "promoted at the watch's GoTo");
    assert_eq!(w.hunted_by.get(&t), Some(&body));
    citysim::systems::lod::set_lod(&mut w, t, Lod::Statistical);
    assert_eq!(lod(&w, t), Some(Lod::Coarse), "never demoted while hunted");
}

fn trace_of(flags: u16, district: citysim::DistrictId, last_day: u64) -> Trace {
    let t = citysim::DayTrace { zone: citysim::Zone::ALL[0], district, flags, hunger: 3, mood: 2 };
    Trace { last_day, days: std::iter::repeat_n(t, 14).collect() }
}

#[test]
fn test_askaround_returns_trace_habit_without_sighting() {
    use citysim::components::trace_flags;
    let mut w = World::new(42, Config::load());
    w.tick = 20 * TICKS_PER_DAY;
    let day = w.day();
    let employed = adults(&w)
        .into_iter()
        .find(|&x| {
            w.comp::<Job>(x).is_some_and(|j| j.employer.is_some() && j.on_shift(12 * 60))
                && w.comp::<citysim::Household>(x).and_then(|h| h.home).is_some()
        })
        .expect("an employed resident on a day shift");
    let home = w.comp::<citysim::Household>(employed).and_then(|h| h.home).unwrap();
    let job = w.comp::<Job>(employed).and_then(|j| j.employer).unwrap();
    let d = w.district_of_building(home);
    w.insert(employed, trace_of(trace_flags::ALIVE | trace_flags::SLEPT_AT_HOME | trace_flags::EMPLOYED, d, day - 1));
    // Night: Home.
    let night = 20 * TICKS_PER_DAY + 23 * TICKS_PER_HOUR;
    let i = hunt::habit(&w, employed, night);
    assert_eq!((i.building, i.source), (Some(home), IntelSource::Habit), "night -> Home");
    // On shift: the workplace.
    let noon = 20 * TICKS_PER_DAY + 12 * TICKS_PER_HOUR;
    assert_eq!(hunt::habit(&w, employed, noon).building, Some(job), "in shift -> workplace");
    // Off shift by day, no job: the busiest Bar of the modal district.
    w.remove::<Job>(employed);
    let bar_d = w.buildings_of_kind(BuildingKind::Bar).iter().map(|&b| w.district_of_building(b)).next().unwrap();
    w.insert(employed, trace_of(trace_flags::ALIVE, bar_d, day - 1));
    let i = hunt::habit(&w, employed, noon);
    assert_eq!(i.building, hunt::busiest_bar(&w, bar_d), "-> the busiest Bar there");
    assert!(i.building.is_some());
}

#[test]
fn test_friend_deceive_sends_hunter_elsewhere() {
    let base = World::new(42, Config::load());
    let bars = base.buildings_of_kind(BuildingKind::Bar).to_vec();
    assert!(bars.len() >= 2);
    let (h, r, t) = three(&base);
    let mut found = false;
    for k in 0..40u64 {
        let mut w = base.clone();
        w.tick = 10 * TICKS_PER_DAY + 13 * TICKS_PER_HOUR + k;
        let venue = bars[0];
        // Nobody else at the venue: the respondent is the target's Friend.
        let others: Vec<EntityId> = w.comp::<citysim::Building>(venue).unwrap().occupants.clone();
        for o in others {
            w.leave_building(o);
        }
        w.enter_building(h, venue);
        w.enter_building(r, venue);
        let e = w.edge_entry(r, t);
        e.kind = RelKind::Friend;
        e.affinity = 0.8;
        if let Some(s) = w.comp_mut::<Skills>(r) {
            s.deception = 1.0;
        }
        if let Some(s) = w.comp_mut::<Skills>(h) {
            s.knowledge = 0.0;
        }
        grudges::add(&mut w, h, t, GrudgeCause::Assaulted, 1.0, 0);
        assert!(hunt::adopt(&mut w, h, HuntWhy::God));
        w.hunts.get_mut(&h).unwrap().venue = Some(venue);
        hunt::ask_around(&mut w, h);
        let s = w.hunts.get(&h).unwrap().clone();
        assert_eq!(s.phase, HuntPhase::Watch);
        if s.deceived {
            let truth = hunt::habit(&w, t, w.tick).building;
            let i = s.intel.unwrap();
            assert_eq!(i.source, IntelSource::Deceived);
            assert_ne!(i.building, truth, "not where the target is");
            assert!(i.building.is_some_and(|b| bars.contains(&b)), "a Bar");
            assert_eq!(s.liar, Some(r));
            found = true;
            break;
        }
    }
    assert!(found, "a Friend with deception 1 lies within 40 tries");
}

#[test]
fn test_revenge_killing_is_murder_and_grudge_witness_silent() {
    let mut cfg = Config::load();
    cfg.crime.witness_base = 1.0;
    let mut w = World::new(42, cfg);
    let ads = adults(&w);
    let guard = w.guards().first().copied().expect("a guard");
    let pick: Vec<EntityId> =
        ads.iter().copied().filter(|&x| x != guard && !w.has::<Job>(x) && w.gang_of(x).is_none()).take(3).collect();
    let (h, t, c) = (pick[0], pick[1], pick[2]);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    for x in [h, c, guard] {
        citysim::systems::lod::set_lod(&mut w, x, Lod::Coarse);
        w.enter_building(x, bar);
    }
    if let Some(s) = w.comp_mut::<Skills>(h) {
        s.stealth = 0.0;
    }
    grudges::add(&mut w, h, t, GrudgeCause::KilledKin(EntityId::NONE), 1.0, 0);
    grudges::add(&mut w, c, t, GrudgeCause::Robbed, 0.4, 0);
    assert!(hunt::adopt(&mut w, h, HuntWhy::God));
    let tile = w.comp::<Position>(h).unwrap().tile;
    assert!(law::street_silence_on(&w, h, Some(t), citysim::Crime::Murder));
    law::raise_crime_on(&mut w, h, None, Some(t), citysim::Crime::Murder, tile);
    let saw = |w: &World, x: EntityId| {
        w.comp::<citysim::Memory>(x)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(h)))
    };
    assert!(!saw(&w, c), "the grudge-holding civilian writes no SawCrime");
    let knows = w.comp::<citysim::Memory>(c).is_some_and(|m| {
        m.heard.iter().any(|e| e.kind == MemoryKind::Rumour && e.deed == Some(Deed::Killed) && e.subject == Some(h))
    });
    assert!(knows, "but knows it");
    assert!(saw(&w, guard), "the guard saw it");
    assert!(law::wanted(&w, h), "the guard files a Murder report");
    assert!(w.stats.current.word.silenced >= 1);
}

#[test]
fn test_kill_chain_makes_next_grudge_chain_plus_one() {
    let mut w = World::new(42, Config::load());
    let (h, t, _) = three(&w);
    grudges::add(&mut w, h, t, GrudgeCause::KilledKin(EntityId::NONE), 1.0, 1);
    assert!(hunt::adopt(&mut w, h, HuntWhy::Goal));
    assert_eq!(w.hunts.get(&h).map(|s| s.chain), Some(1));
    // A won strike that did not kill: the target hears it first-hand.
    hunt::on_strike(&mut w, h, t, h, false);
    assert_eq!(w.kill_chain.get(&t).map(|x| x.0), Some(2), "kill_chain = chain + 1");
    assert!(!w.hunts.contains_key(&h), "the Hunt ended");
    let g = grudge(&w, t, h).expect("the beaten target's grudge on the avenger");
    assert_eq!(g.chain, 2, "one link deeper");
    assert_eq!(w.stats.current.word.avenged, 1);
    // The target's kin hearing of it start at the same chain.
    let s = adults(&w).into_iter().find(|&x| x != h && x != t && w.edge(x, t).is_none() && w.edge(x, h).is_none());
    let s = s.unwrap();
    w.set_spouse(s, t);
    let e = MemoryEntry {
        subject: Some(h),
        salience: 0.9,
        deed: Some(Deed::Avenged),
        object: Some(t),
        hops: 1,
        ..MemoryEntry::blank(MemoryKind::Rumour, w.tick)
    };
    memory::hear_entry(&mut w, s, e);
    assert_eq!(grudge(&w, s, h).map(|g| g.chain), Some(2));
}

#[test]
fn test_vendetta_opens_and_retaliate_targets_it() {
    let mut cfg = Config::load();
    cfg.gangs.order_flat.retaliate = 5.0;
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10);
    let gang = led_gang(&mut w);
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.exec.is_some() && !cc.buildings.is_empty()))
        .expect("a corp with an exec");
    w.push_command(PlayerCommand::DeclareVendetta { a: corp, b: gang, weight: 1.0 });
    w.apply_commands();
    assert!(w.vendettas.iter().any(|v| (v.a, v.b) == (corp.min(gang), corp.max(gang))), "{:?}", w.vendettas);
    assert_eq!(grudges::vendetta_for(&w, gang).map(|x| x.0), Some(corp));
    let i = faction::gather_inputs(&w, gang).expect("a leader");
    assert!(i.grudge && i.vendetta.is_some());
    let scores = faction::score_orders(&i, &w.config.gangs);
    assert!(scores.iter().any(|s| s.order == Order::Retaliate), "Retaliate scores");
    if let Some(g) = w.comp_mut::<citysim::Gang>(gang) {
        g.last_raid_tick = None;
        g.raid_at = None;
    }
    faction::rescore(&mut w, gang, 0.0);
    let g = w.comp::<citysim::Gang>(gang).unwrap().clone();
    assert_eq!(g.order, Order::Retaliate);
    assert_eq!(g.retaliate_on, Some(corp));
    assert_eq!(raid::expedition_rival(&w, gang), Some(corp));
    let target = raid::corp_target(&w, gang).expect("a corp building");
    assert_eq!(w.owner_of(target), Some(corp), "the raid goes to its building");
}

#[test]
fn test_contract_lost_on_honour() {
    let mut w = World::new(42, Config::load());
    let sec: Vec<EntityId> = w
        .corps()
        .into_iter()
        .filter(|&c| w.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&citysim::Niche::Security)))
        .collect();
    assert!(sec.len() >= 2, "two Security corps are seeded");
    let (a, b) = (sec[0], sec[1]);
    for c in [a, b] {
        w.comp_mut::<Corp>(c).unwrap().price_level.insert(citysim::Niche::Security, 1.0);
        w.comp_mut::<Corp>(c).unwrap().contracts.clear();
    }
    let set = |w: &mut World, c: EntityId, honour: f32| {
        let i = c.index as usize;
        w.reputation[i] = Some(Reputation { honour, ..Reputation::default() });
    };
    set(&mut w, a, 0.2);
    set(&mut w, b, 0.8);
    assert!(citysim::systems::corps::capacity(&w, b) > 0, "the honest corp has room");
    let (to, by_honour) = citysim::systems::corps::honour_seller(&w, a);
    assert_eq!(to, Some(b), "the dishonoured corp loses the renewal");
    assert!(by_honour, "at equal price: on honour");
    let (stay, _) = citysim::systems::corps::honour_seller(&w, b);
    assert_eq!(stay, None, "the honest corp keeps its client");
}

#[test]
fn test_guard_body_turns_strip_into_fight() {
    let mut w = World::new(42, Config::load());
    let (dead, g, s) = three(&w);
    w.edge_entry(g, dead).kind = RelKind::Family;
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    for x in [dead, g, s] {
        citysim::systems::lod::set_lod(&mut w, x, Lod::Coarse);
        w.enter_building(x, bar);
    }
    w.kill_by(dead, DeathCause::Violence, Some(s));
    assert!(w.comp::<citysim::Corpse>(dead).is_some());
    let plan = grudges::guard_body_plan(&mut w, g);
    // The guard saw the body die (SawCorpse) or not: its plan is set by hand.
    let plan = plan.unwrap_or(Plan {
        goal: GoalKind::GuardBody,
        target: Some(dead),
        steps: vec![citysim::ActionInstance { action: ActionKind::StakeOut, target: Some(dead), tile: None }],
        started_tick: w.tick,
    });
    let b = w.comp_mut::<Brain>(g).unwrap();
    b.plan = Some(Plan { steps: plan.steps[plan.steps.len() - 1..].to_vec(), ..plan });
    b.plan_step = 0;
    b.current_goal = Some(GoalKind::GuardBody);
    let now = w.tick;
    w.comp_mut::<Brain>(g).unwrap().exec =
        ExecState::Use { kind: ActionKind::StakeOut, until: now + 360, started: now };
    grudges::start_guard(&mut w, g, dead);
    assert!(grudges::guarding(&w, g, dead));
    let fought = |w: &World, x: EntityId, o: EntityId| {
        w.comp::<citysim::Memory>(x)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Fought && e.subject == Some(o)))
    };
    assert!(!fought(&w, s, g) && !fought(&w, g, s));
    let go_on = grudges::guard_contests(&mut w, s, dead);
    // Fought on both sides (one may have died of it, losing its memory).
    assert!(fought(&w, s, g) || fought(&w, g, s), "the strip became a fight");
    let won = w
        .comp::<citysim::Memory>(s)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::Won && e.subject == Some(g)));
    assert_eq!(go_on, won, "the strip goes on only if the stripper won");
    assert_eq!(w.stats.current.word.guard_body, 1);
    // No guard: the strip goes on unopposed.
    let (_, _, x) = three(&w);
    assert!(grudges::guard_contests(&mut w, x, x));
}

/// Fix round: one deed leaves one grudge. A noticing victim is also a
/// witness (`SawCrime` with itself as the object) and keeps its
/// `WasRobbed`; only the latter may form the grudge: sev 0.6 × own 0.6
/// (M15 phase 5; it was 0.3 × 0.8).
#[test]
fn test_noticing_victim_holds_one_grudge_per_deed() {
    let mut cfg = Config::load();
    cfg.crime.witness_base = 1.0;
    let mut w = World::new(42, cfg);
    let (thief, victim, _) = three(&w);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    for x in [thief, victim] {
        citysim::systems::lod::set_lod(&mut w, x, Lod::Coarse);
        w.enter_building(x, bar);
    }
    let tile = w.comp::<Position>(thief).unwrap().tile;
    law::raise_crime(&mut w, thief, Some(victim), citysim::Crime::Theft, tile);
    let saw = w.comp::<citysim::Memory>(victim).is_some_and(|m| {
        m.entries.iter().any(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(thief) && e.object == Some(victim))
    });
    assert!(saw, "the victim noticed its own robbery");
    let g = grudge(&w, victim, thief).expect("a grudge on the thief");
    assert!((g.weight - 0.36).abs() < 1e-5, "one deed, one grudge: {}", g.weight);
}
