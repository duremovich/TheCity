//! M17 phase 5 (docs/M17_OUTSIDE.md § 5, § 6): missions abroad at the
//! Ledger LOD, their transient cast and its promotion, the Blackwall, the
//! outside's traces and `SetStateGuards`. A crew is away (Statistical,
//! skipped by the hourly table) until `back_at`; one keyed draw settles the
//! mission (stable across a save and a re-run), kills only abroad, and
//! promotes only past the thresholds; a promoted NPC arrives with its
//! stored name and its edge. Every crossing keeps the coin identity.

use citysim::components::{Brain, Lod, Skills};
use citysim::outside::OutsideAsset;
use citysim::systems::{abroad, econ, lod, parents};
use citysim::{Config, Corp, EntityId, EventKind, PlayerCommand, RelKind, World, TICKS_PER_DAY};

fn account(w: &World, name: &str) -> u32 {
    w.outside.factions.iter().find(|f| f.name == name).map(|f| f.id).unwrap_or_else(|| panic!("no account {name}"))
}

fn city() -> World {
    let mut w = World::new(42, Config::load());
    w.run_ticks(2 * TICKS_PER_DAY + 30);
    w
}

fn god(w: &mut World, cmd: PlayerCommand) {
    w.push_command(cmd);
    w.apply_commands();
}

/// `n` free adults with no job, strong or weak.
fn crew(w: &mut World, n: usize, fighting: f32) -> Vec<EntityId> {
    let c: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&id| {
            citysim::systems::contracts::free_adult(w, id)
                && !w.has::<citysim::Job>(id)
                && !citysim::systems::contracts::holds_body(w, id)
        })
        .take(n)
        .collect();
    for &a in &c {
        w.comp_mut::<Skills>(a).expect("skills").fighting = fighting;
        w.comp_mut::<citysim::Personality>(a).expect("p").courage = fighting;
    }
    c
}

fn count(w: &World, k: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == k).count()
}

fn days(w: &mut World, d: u64) {
    w.run_ticks(d * TICKS_PER_DAY);
}

#[test]
fn test_crew_is_away_then_comes_back() {
    let mut w = city();
    let on = account(&w, "Arasaka Global");
    let team = crew(&mut w, 4, 0.8);
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::Convoy, crew: team.clone() });
    assert_eq!(w.outside.abroad.len(), 1);
    for &a in &team {
        assert!(abroad::is_away(&w, a));
        assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical));
        assert!(!citysim::systems::contracts::free_adult(&w, a), "no record takes a crew member away");
    }
    assert_eq!(count(&w, EventKind::AbroadDeparted), 1);
    // A day on: still away, kept Statistical by the hourly tiers, and the
    // hourly table never runs them.
    days(&mut w, 1);
    for &a in &team {
        assert!(abroad::is_away(&w, a));
        assert_eq!(w.comp::<Brain>(a).map(|b| b.lod), Some(Lod::Statistical));
    }
    let due: Vec<EntityId> = (0..60).flat_map(|_| lod::due_this_tick(&w)).collect();
    assert!(team.iter().all(|a| !due.contains(a)));
    let d = u64::from(w.config.outside.abroad.abroad_days) + 1;
    days(&mut w, d);
    assert!(w.outside.abroad.is_empty() && w.outside.away.is_empty(), "back");
    assert_eq!(count(&w, EventKind::AbroadResolved), 1);
    for &a in team.iter().filter(|&&a| citysim::systems::law::living(&w, a)) {
        assert!(!abroad::is_away(&w, a));
    }
}

/// The draw is the same on a twin, and on a save loaded mid-mission.
#[test]
fn test_draw_is_stable_across_save_load_and_rerun() {
    let mut w = city();
    let on = account(&w, "Militech International");
    let team = crew(&mut w, 3, 0.5);
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::Depot, crew: team });
    let mut twin = w.clone();
    let mut loaded = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("loads");
    let d = u64::from(w.config.outside.abroad.abroad_days) + 1;
    for x in [&mut w, &mut twin, &mut loaded] {
        days(x, d);
    }
    let said = |x: &World| {
        x.events.iter().filter(|e| e.kind == EventKind::AbroadResolved).map(|e| e.text.clone()).collect::<Vec<_>>()
    };
    assert_eq!(said(&w).len(), 1);
    assert_eq!(said(&w), said(&twin));
    assert_eq!(said(&w), said(&loaded));
    assert_eq!(citysim::save::to_ron(&w), citysim::save::to_ron(&loaded));
}

/// A strong crew takes the depot: the coins cross in from the parent (the
/// identity holds); the crew's share is paid.
#[test]
fn test_won_depot_pays_and_keeps_the_identity() {
    let mut w = city();
    w.config.outside.abroad.death_p = 0.0;
    let on = account(&w, "Arasaka Global");
    let team = crew(&mut w, 6, 1.0);
    let buyer = w.gangs()[0];
    let wallets = |x: &World| team.iter().map(|&a| x.purse(Some(a))).sum::<i64>();
    let (g0, c0, t0) = (w.purse(Some(buyer)), wallets(&w), w.outside.faction(on).expect("p").treasury);
    let base = econ::identity(&w);
    god(&mut w, PlayerCommand::PostAbroad { buyer: Some(buyer), on, asset: OutsideAsset::Depot, crew: team.clone() });
    let m = w.outside.abroad[0].clone();
    assert!(abroad::odds(&w, &m) > 0.9, "a strong crew: {}", abroad::odds(&w, &m));
    w.run_ticks((m.back_at - w.tick) + TICKS_PER_DAY);
    let ev = w.events.iter().find(|e| e.kind == EventKind::AbroadResolved).expect("resolved");
    assert!(ev.text.contains("coins taken"), "{}", ev.text);
    assert!(w.purse(Some(buyer)) > g0, "the buyer's cut");
    assert!(wallets(&w) > c0, "the crew's share");
    assert!(w.outside.faction(on).expect("p").treasury < t0 + 10_000, "paid from the parent");
    assert_eq!(econ::identity(&w), base, "coins only cross");
}

/// A weak crew sure to lose dies only abroad: the dead are buried there,
/// the rest come home.
#[test]
fn test_deaths_happen_only_abroad() {
    let mut w = city();
    w.config.outside.abroad.death_p = 1.0;
    let on = account(&w, "Arasaka Global");
    let team = crew(&mut w, 4, 0.0);
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::DataCentre, crew: team.clone() });
    // Away, nobody dies: the mission runs its days.
    days(&mut w, 1);
    assert!(team.iter().all(|&a| citysim::systems::law::living(&w, a)));
    let d = u64::from(w.config.outside.abroad.abroad_days) + 1;
    days(&mut w, d);
    let dead: Vec<EntityId> = team.iter().copied().filter(|&a| !citysim::systems::law::living(&w, a)).collect();
    assert!(!dead.is_empty(), "a hopeless job costs lives");
    for a in dead {
        // Buried abroad (a buried body is freed a day on): none lies in the city.
        assert!(w.comp::<citysim::Corpse>(a).is_none_or(|c| c.buried), "buried abroad, no body in the city");
    }
    assert!(
        w.outside.today.abroad_dead > 0
            || w.stats.history.iter().any(|r| r.outside.as_ref().is_some_and(|o| o.abroad_dead > 0))
    );
}

/// Promotion only past the thresholds and under the roll; the promoted
/// arrive with the stored name and the edge to the crew's leader.
#[test]
fn test_promotion_past_the_thresholds_arrives_with_the_edge() {
    let run = |friend: f32, enemy: f32, p: f32| {
        let mut w = city();
        let cfg = &mut w.config.outside.abroad;
        (cfg.promote_friend, cfg.promote_enemy, cfg.promote_p, cfg.promote_delay_days, cfg.death_p) =
            (friend, enemy, p, 1, 0.0);
        let on = account(&w, "Arasaka Global");
        let team = crew(&mut w, 3, 1.0);
        god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::Convoy, crew: team.clone() });
        let d = u64::from(w.config.outside.abroad.abroad_days) + 3;
        days(&mut w, d);
        (w, team[0])
    };
    // Every disposition past the Friend threshold, the roll certain.
    let (w, leader) = run(-1.0, -2.0, 1.0);
    let arrived: Vec<_> = w.events.iter().filter(|e| e.kind == EventKind::CastArrived).collect();
    assert!(!arrived.is_empty(), "a promoted NPC arrived");
    let max = w.config.outside.abroad.promoted_max as usize;
    assert!(arrived.len() <= max);
    let a = arrived[0].actors[0];
    assert_eq!(arrived[0].actors.get(1), Some(&leader));
    assert!(w.edge(a, leader).is_some_and(|e| e.kind == RelKind::Friend && e.affinity > 0.5));
    assert!(w.outside.promoted_living.contains(&a));
    assert!(arrived[0].text.starts_with(&w.name_of(a)), "the stored name");
    // Every disposition an Enemy: the arrival holds a grudge.
    let (w, leader) = run(2.0, 1.0, 1.0);
    let e = w.events.iter().find(|e| e.kind == EventKind::CastArrived).expect("an enemy arrived");
    let a = e.actors[0];
    assert!(w.edge(a, leader).is_some_and(|e| e.kind == RelKind::Enemy));
    assert!(citysim::systems::grudges::grudge_on(&w, a, leader).is_some());
    // Inside the thresholds, or the roll at 0: nobody.
    let (w, _) = run(2.0, -2.0, 1.0);
    assert_eq!(count(&w, EventKind::CastArrived), 0);
    let (w, _) = run(-1.0, -2.0, 0.0);
    assert_eq!(count(&w, EventKind::CastArrived), 0);
}

/// A breach lands at the next midnight, flatlines the runner, wipes the
/// parent's Data; a second inside the cooldown does not.
#[test]
fn test_blackwall_breach_and_its_cooldown() {
    let mut w = city();
    w.config.outside.blackwall.breach_p = 1.0;
    let on = account(&w, "Arasaka Global");
    let runner = crew(&mut w, 1, 0.5)[0];
    abroad::on_uplink_run(&mut w, on, runner, None);
    assert!(w.outside.breach_due.is_some(), "rolled");
    days(&mut w, 1);
    assert_eq!(count(&w, EventKind::BlackwallBreach), 1);
    assert!(!citysim::systems::law::living(&w, runner), "flatlined");
    let p = w.outside.factions.iter().find(|f| f.id == on).and_then(|f| f.parent.clone()).expect("p");
    assert!(p.data.units.iter().sum::<u32>() <= 3 * 10, "the Data abroad wiped (a day's regrowth at most)");
    let second = crew(&mut w, 1, 0.5)[0];
    abroad::on_uplink_run(&mut w, on, second, None);
    assert!(w.outside.breach_due.is_none(), "inside the cooldown");
    days(&mut w, 1);
    assert_eq!(count(&w, EventKind::BlackwallBreach), 1);
    assert!(citysim::systems::law::living(&w, second));
}

/// The outside traces: a sighting at `trace_conf` in each branch's database.
#[test]
fn test_the_outside_traces_into_its_branches() {
    let mut w = city();
    let on = account(&w, "Arasaka Global");
    let who = crew(&mut w, 1, 0.5)[0];
    abroad::trace(&mut w, on, who);
    let branch =
        w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.parent == Some(on))).expect("branch");
    let db = w.db.get(&branch).expect("a database");
    let conf = w.config.outside.blackwall.trace_conf;
    assert!(db.sightings.iter().any(|s| s.who == who && (s.confidence - conf).abs() < 1e-6));
}

#[test]
fn test_set_state_guards_caps_the_state() {
    let mut w = city();
    w.outside.state_guards = 12;
    god(&mut w, PlayerCommand::SetStateGuards(Some(4)));
    assert_eq!(w.outside.state_guards, 4);
    parents::state_act(&mut w, citysim::outside::OutsideOrder::Fund);
    assert!(w.outside.state_guards <= 4, "Fund stops at the city's cap");
    god(&mut w, PlayerCommand::SetStateGuards(Some(0)));
    assert_eq!(w.outside.state_guards, 0, "the State's men refused");
    god(&mut w, PlayerCommand::SetStateGuards(None));
    assert_eq!(parents::state_cap(&w), w.config.outside.parents.state_guards_max);
}

#[test]
fn test_god_found_parent() {
    let mut w = city();
    let corp =
        w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.parent.is_none())).expect("a local corp");
    let n = w.outside.factions.len();
    god(&mut w, PlayerCommand::FoundParent(corp));
    assert_eq!(w.outside.factions.len(), n + 1);
    assert!(w.comp::<Corp>(corp).and_then(|c| c.parent).is_some());
    assert!(parents::god_found_parent(&mut w, corp).is_err(), "a branch already");
}
