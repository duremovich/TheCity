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
    // Jobs & lives phase 3: M17's missions as written, without the weapons
    // (their butterfly had the first Enemy arrival attack the unarmed leader
    // and die on the first day, so the grudge the test reads was gone).
    let mut c = Config::load();
    c.weapons.enabled = false;
    let mut w = World::new(42, c);
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
    // Review 7: every slot of the hour (the tick advanced), not one slot 60 times.
    let mut due: Vec<EntityId> = Vec::new();
    for _ in 0..60 {
        due.extend(lod::due_this_tick(&w));
        w.run_ticks(1);
    }
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
    let _ = t0;
    // Review: the settling alone, so the parent's treasury moves by the loot only.
    w.outside.abroad[0].back_at = w.tick;
    let (g1, c1, t1) = (w.purse(Some(buyer)), wallets(&w), w.outside.faction(on).expect("p").treasury);
    abroad::daily(&mut w);
    let ev = w.events.iter().find(|e| e.kind == EventKind::AbroadResolved).expect("resolved");
    assert!(ev.text.contains("coins taken"), "{}", ev.text);
    let (buyer_got, crew_got) = (w.purse(Some(buyer)) - g1, wallets(&w) - c1);
    assert!(buyer_got > 0 && w.purse(Some(buyer)) > g0, "the buyer's cut");
    assert!(crew_got > 0 && wallets(&w) > c0, "the crew's share");
    let paid = t1 - w.outside.faction(on).expect("p").treasury;
    assert_eq!(paid, buyer_got + crew_got, "paid from the parent's treasury, to the coin");
    assert_eq!(paid, w.config.outside.abroad.loot, "the whole loot");
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
    // Away, nobody dies while the mission runs its days (review: every hour
    // until it settles), and at the settling's tick every dead crew member
    // is already buried (review 8: read before a buried body is freed).
    while count(&w, EventKind::AbroadResolved) == 0 {
        assert!(team.iter().all(|&a| citysim::systems::law::living(&w, a)), "a death while away");
        w.run_ticks(60);
    }
    let dead: Vec<EntityId> = team.iter().copied().filter(|&a| !citysim::systems::law::living(&w, a)).collect();
    assert!(!dead.is_empty(), "a hopeless job costs lives");
    for a in dead {
        assert!(w.comp::<citysim::Corpse>(a).is_some_and(|c| c.buried), "buried abroad, no body in the city");
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
        while count(&w, EventKind::AbroadResolved) == 0 {
            w.run_ticks(60);
        }
        let names: Vec<String> = w.outside.promoted.iter().map(|p| p.cast.name.clone()).collect();
        days(&mut w, 3);
        (w, team[0], names)
    };
    // Every disposition past the Friend threshold, the roll certain.
    let (w, leader, names) = run(-1.0, -2.0, 1.0);
    let arrived: Vec<_> = w.events.iter().filter(|e| e.kind == EventKind::CastArrived).collect();
    assert!(!arrived.is_empty(), "a promoted NPC arrived");
    let max = w.config.outside.abroad.promoted_max as usize;
    assert!(arrived.len() <= max);
    let a = arrived[0].actors[0];
    assert_eq!(arrived[0].actors.get(1), Some(&leader));
    assert!(w.edge(a, leader).is_some_and(|e| e.kind == RelKind::Friend && e.affinity > 0.5));
    assert!(w.outside.promoted_living.contains(&a));
    // Review 9: the arrival carries the Promoted record's name.
    assert!(!names.is_empty());
    assert!(arrived.iter().all(|e| names.contains(&w.name_of(e.actors[0]))), "the stored names: {names:?}");
    // Every disposition an Enemy: the arrival holds a grudge.
    let (w, leader, _) = run(2.0, 1.0, 1.0);
    // Jobs & lives phase 8: an arrival still living (a dead one's grudges
    // and edges go with its components; on seed 42 the first of six bled
    // out after the fight with the leader).
    let e = w
        .events
        .iter()
        .filter(|e| e.kind == EventKind::CastArrived)
        .find(|e| citysim::systems::law::living(&w, e.actors[0]))
        .expect("an enemy arrived and lives");
    let a = e.actors[0];
    assert!(w.edge(a, leader).is_some_and(|e| e.kind == RelKind::Enemy));
    // Jobs & lives phase 7b: one of the six arrivals killed the leader 70
    // ticks after they came (seed 42 under the mirrored off-screen evening),
    // and a target's death settles the grudges on it: the arrival swore the
    // grudge (an Abroad entry on the leader), standing while the leader lives.
    let sworn = w
        .comp::<citysim::word::Grudges>(a)
        .is_some_and(|g| g.list.iter().any(|x| x.target == leader && x.cause == citysim::word::GrudgeCause::Abroad));
    assert!(sworn, "the arrival swore a grudge on the leader");
    if citysim::systems::law::living(&w, leader) {
        assert!(citysim::systems::grudges::grudge_on(&w, a, leader).is_some());
    }
    // Inside the thresholds, or the roll at 0: nobody.
    let (w, _, _) = run(2.0, -2.0, 1.0);
    assert_eq!(count(&w, EventKind::CastArrived), 0);
    let (w, _, _) = run(-1.0, -2.0, 0.0);
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
    let traced = |w: &World, x: EntityId| {
        w.db.get(&branch).is_some_and(|d| d.sightings.iter().any(|s| s.who == x && (s.confidence - conf).abs() < 1e-6))
    };
    // Review: a run that lands on the uplink is traced.
    let runner = crew(&mut w, 1, 0.5)[0];
    parents::uplink_landed(&mut w, on, citysim::virt::Purpose::Data { wipe: false }, 0, None, runner);
    assert!(traced(&w, runner), "uplink_landed traces the runner");
    // Review: the known killer of one of the parent's Outsiders is traced.
    parents::force_reinforce(&mut w, on, 1).expect("an Outsider");
    let outsider = w.outside.outsiders.iter().find(|(_, o)| o.from == on && !o.brother).map(|(&a, _)| a).expect("one");
    let killer = crew(&mut w, 1, 0.5)[0];
    w.kill_by(outsider, citysim::DeathCause::Violence, Some(killer));
    assert!(traced(&w, killer), "kill_by traces the killer");
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
    // Review extra: a corp short of `found_parent_treasury` is refused (a
    // parent founded on nothing would starve).
    w.comp_mut::<Corp>(corp).expect("corp").treasury = 100;
    assert!(parents::god_found_parent(&mut w, corp).is_err(), "a broke corp founds no parent");
    w.comp_mut::<Corp>(corp).expect("corp").treasury = w.config.outside.parents.found_parent_treasury * 2;
    let n = w.outside.factions.len();
    god(&mut w, PlayerCommand::FoundParent(corp));
    assert_eq!(w.outside.factions.len(), n + 1);
    assert!(w.comp::<Corp>(corp).and_then(|c| c.parent).is_some());
    assert!(parents::god_found_parent(&mut w, corp).is_err(), "a branch already");
}
