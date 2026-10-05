//! M8: factions. The brain's orders, claims and flips, the raid resolver.

use citysim::systems::faction::{self, OrderInputs};
use citysim::systems::gang;
use citysim::{
    ActionKind, Brain, Building, BuildingKind, Claim, Config, EntityId, EventKind, Gang, LocationKey, Needs, Order,
    Personality, Shock, World,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

/// Two jobless adults who are not each other's spouse.
fn two_civilians(w: &World) -> (EntityId, EntityId) {
    let mut it = w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<Brain>(id));
    let a = it.next().expect("a");
    let b = it.find(|&b| w.spouse_of(a) != Some(b) && w.edge(a, b).is_none()).expect("b");
    (a, b)
}

fn gangs(w: &World) -> (EntityId, EntityId) {
    let g = w.gangs();
    (g[0], g[1])
}

/// A leader with middling traits, so the tests pin the curves, not the dice.
fn temper(w: &mut World, id: EntityId) {
    let p = w.comp_mut::<Personality>(id).expect("p");
    p.greed = 0.5;
    p.courage = 0.5;
    p.pride = 0.5;
    p.loyalty = 0.9;
}

fn inputs() -> OrderInputs {
    OrderInputs {
        frontier: 20,
        frontier_total: 30,
        rival_territory: 0,
        own: 5,
        rival: 5,
        heat: 0.0,
        prize: 0,
        grudge: false,
        greed: 0.5,
        courage: 0.5,
        pride: 0.5,
        raid_ready: true,
        rival_exists: true,
        sacked: false,
        jailed: 0,
        boss_jailed: false,
        breakout_ready: true,
        garrison: false,
        loyalty: 0.5,
        hoard: 0.0,
        hoard_corp: None,
        hoard_tilt: 0.1,
        target_cover: 0.0,
        jail_cover: 0.0,
    }
}

fn best(i: &OrderInputs) -> Order {
    faction::score_orders(i, &Config::load().gangs)[0].order
}

#[test]
fn test_order_expand_while_the_frontier_is_open() {
    assert_eq!(best(&inputs()), Order::Expand);
}

#[test]
fn test_order_contest_at_parity_once_the_frontier_is_claimed() {
    let i = OrderInputs { frontier: 0, rival_territory: 5, ..inputs() };
    assert_eq!(best(&i), Order::Contest);
}

#[test]
fn test_order_raid_with_a_strength_edge_and_a_prize() {
    let i = OrderInputs { frontier: 0, rival_territory: 5, own: 10, rival: 5, prize: 200, courage: 0.8, ..inputs() };
    assert_eq!(best(&i), Order::Raid);
    let broke = OrderInputs { prize: 5, ..i };
    assert_ne!(best(&broke), Order::Raid, "no prize, no raid");
}

#[test]
fn test_order_retaliate_on_a_grudge_regardless_of_ratio() {
    let i = OrderInputs { grudge: true, own: 2, rival: 8, ..inputs() };
    assert_eq!(best(&i), Order::Retaliate);
    let sacked = OrderInputs { sacked: true, raid_ready: false, ..i };
    assert_ne!(best(&sacked), Order::Retaliate, "a sacked gang cannot muster");
    let cooling = OrderInputs { raid_ready: false, ..i };
    assert_ne!(best(&cooling), Order::Retaliate, "one raid per cooldown, grudge or not");
}

#[test]
fn test_order_lie_low_under_heat() {
    let i = OrderInputs { heat: 0.6, ..inputs() };
    assert_eq!(best(&i), Order::LieLow);
}

#[test]
fn test_order_break_out_for_the_boss_or_lie_low_when_timid() {
    // Two inside, the boss among them, a brave loyal leader: break them out.
    let i = OrderInputs { jailed: 2, boss_jailed: true, courage: 0.8, loyalty: 0.8, heat: 0.4, ..inputs() };
    assert_eq!(best(&i), Order::BreakOut);
    // The same gang under a timid leader and more heat lies low instead.
    let timid = OrderInputs { courage: 0.2, loyalty: 0.3, heat: 0.6, ..i.clone() };
    assert_eq!(best(&timid), Order::LieLow);
    // The cooldown, the headcount and an empty Jail each gate it.
    assert_ne!(best(&OrderInputs { breakout_ready: false, ..i.clone() }), Order::BreakOut);
    assert_ne!(best(&OrderInputs { own: 1, ..i.clone() }), Order::BreakOut);
    assert_ne!(best(&OrderInputs { jailed: 0, boss_jailed: false, ..i.clone() }), Order::BreakOut);
    // Without the boss inside a lone grunt is not worth the Jail's guards.
    let grunt = OrderInputs { jailed: 1, boss_jailed: false, courage: 0.5, loyalty: 0.5, heat: 0.0, ..i };
    assert_ne!(best(&grunt), Order::BreakOut);
}

#[test]
fn test_choose_respects_hysteresis() {
    let cfg = Config::load().gangs;
    let scores = faction::score_orders(&inputs(), &cfg);
    // Expand leads comfortably here: a current Expand holds, a current LieLow does not.
    assert_eq!(faction::choose(&scores, Order::Expand, 0.1), None);
    assert_eq!(faction::choose(&scores, Order::LieLow, 0.1), Some(Order::Expand));
    // Hand-built: a lead inside the band holds at the daily rescoring and switches on a shock.
    let close = vec![
        citysim::OrderScore { order: Order::Contest, score: 0.50, considerations: vec![] },
        citysim::OrderScore { order: Order::Expand, score: 0.45, considerations: vec![] },
    ];
    assert_eq!(faction::choose(&close, Order::Expand, 0.1), None);
    assert_eq!(faction::choose(&close, Order::Expand, 0.0), Some(Order::Contest));
}

#[test]
fn test_next_muster_is_at_least_two_hours_away() {
    assert_eq!(faction::next_muster(0, 22), 1320);
    assert_eq!(faction::next_muster(1200, 22), 1320);
    assert_eq!(faction::next_muster(1300, 22), 1320 + 1440);
}

#[test]
fn test_two_gangs_seeded_with_their_own_hideouts() {
    let w = world(42);
    let (g0, g1) = gangs(&w);
    let h0 = w.hideout_of(g0).expect("h0");
    let h1 = w.hideout_of(g1).expect("h1");
    assert_ne!(h0, h1);
    for h in [h0, h1] {
        assert_eq!(w.comp::<Building>(h).expect("b").kind, BuildingKind::Hideout);
    }
    assert_eq!(w.comp::<Gang>(g0).expect("g").name, "The Hollow");
    assert_eq!(w.comp::<Gang>(g1).expect("g").name, "Ninefold");
    assert_eq!(w.rival_of(g0), Some(g1));
    assert_eq!(w.rival_of(g1), Some(g0));
}

#[test]
fn test_shocks_force_a_midday_rescore() {
    let mut w = world(43);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    assert_eq!(w.comp::<Gang>(g0).expect("g").leader, Some(a));
    w.tick = 700; // noon: not the daily rescoring
    gang::push_shock(&mut w, g0, Shock::MemberKilled { by_rival: true });
    gang::run(&mut w);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Retaliate);
    assert!(g.shocks.is_empty(), "shocks are consumed");
    assert_eq!(g.raid_at, Some(1320), "musters at 22:00 tonight");
    assert!(g.retaliate_until.is_some());
    assert_eq!(g.order_trace.first().map(|s| s.order), Some(Order::Retaliate));
    assert!(w.events.iter().any(|e| e.kind == EventKind::OrderChanged && e.text.contains("shock")));
}

#[test]
fn test_no_leader_no_new_orders() {
    let mut w = world(44);
    let (g0, _) = gangs(&w);
    w.tick = 700;
    gang::push_shock(&mut w, g0, Shock::Sacked);
    gang::run(&mut w);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Expand);
    assert!(g.order_trace.is_empty());
    assert!(!w.events.iter().any(|e| e.kind == EventKind::OrderChanged));
}

#[test]
fn test_claim_resets_on_a_rival_blow_and_flips_at_three() {
    let mut w = world(45);
    let (g0, g1) = gangs(&w);
    let (a, b) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    gang::enlist(&mut w, b, g1);
    let homes = [a, b].map(|x| w.comp::<citysim::Household>(x).and_then(|h| h.home));
    let home = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .copied()
        .find(|&h| !homes.contains(&Some(h)) && w.comp::<Building>(h).is_some_and(|bd| bd.occupants.len() >= 2))
        .expect("an occupied home");
    for _ in 0..3 {
        gang::extort(&mut w, a, home);
    }
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 3 }));
    assert!(w.comp::<Gang>(g0).expect("g").territory.contains(&home));
    assert!(!w.events.iter().any(|e| e.kind == EventKind::TerritoryFlipped), "a first claim is not a flip");

    gang::extort(&mut w, b, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g1, count: 1 }), "a rival blow resets");
    assert!(w.comp::<Gang>(g0).expect("g").territory.contains(&home), "still held until the third blow");
    assert_eq!(w.edge(a, b).map(|e| e.kind), Some(citysim::RelKind::Enemy), "the holders hate the intruder");

    gang::extort(&mut w, b, home);
    gang::extort(&mut w, b, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g1, count: 3 }));
    assert!(w.comp::<Gang>(g1).expect("g").territory.contains(&home));
    assert!(!w.comp::<Gang>(g0).expect("g").territory.contains(&home));
    assert!(w.events.iter().any(|e| e.kind == EventKind::TerritoryFlipped && e.text.contains("from The Hollow")));
    assert!(w.comp::<Gang>(g0).expect("g").shocks.contains(&Shock::HomeFlippedAgainst));
}

#[test]
fn test_gang_work_target_follows_the_order() {
    let mut w = world(46);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let (_, following) = gang::gang_work_target(&w, a).expect("an unclaimed Home to expand into");
    assert_eq!(following, Some(Order::Expand));
    w.comp_mut::<Gang>(g0).expect("g").order = Order::Contest;
    assert!(gang::gang_work_target(&w, a).is_none(), "nothing of the rival's to contest yet");
    w.comp_mut::<Gang>(g0).expect("g").order = Order::LieLow;
    assert!(gang::gang_work_target(&w, a).is_none(), "lying low");
    w.comp_mut::<Personality>(a).expect("p").loyalty = 0.1;
    let (_, following) = gang::gang_work_target(&w, a).expect("a freelancer works anyway");
    assert_eq!(following, None);
}

#[test]
fn test_sacked_gang_does_not_recruit() {
    let mut w = world(47);
    let (g0, _) = gangs(&w);
    let (recruit, member) = two_civilians(&w);
    gang::enlist(&mut w, member, g0);
    let contact = w.config.social.join_gang_affinity + 0.05;
    w.edge_entry(recruit, member).affinity = contact;
    assert_eq!(gang::recruit_gang(&w, recruit), Some(g0));
    w.comp_mut::<Gang>(g0).expect("g").sacked_until = Some(w.tick + 1000);
    assert_eq!(gang::recruit_gang(&w, recruit), None);
}

#[test]
fn test_legacy_save_migrates_hideout_and_claims() {
    let mut w = world(48);
    let (g0, _) = gangs(&w);
    let hideout = w.hideout_of(g0).expect("h");
    let home = w.buildings_by_kind[&BuildingKind::Home][0];
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.hideout = EntityId::NONE;
        g.territory = vec![hideout, home];
        g.territory.sort();
    }
    w.migrate_legacy();
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.hideout, hideout);
    assert_eq!(g.territory, vec![home]);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 3 }));
}

// ---------------------------------------------------------------------------
// Raids
// ---------------------------------------------------------------------------

use citysim::systems::raid::{self, Outcome};
use citysim::{GoalKind, Skills};

/// Put a member on a street tile with the Raid goal, as a raider who has marched.
fn stage_raider(w: &mut World, id: EntityId, tile: citysim::TilePos) {
    w.abort_plan(id);
    w.leave_building(id);
    let p = w.comp_mut::<citysim::Position>(id).expect("pos");
    p.tile = tile;
    p.building = None;
    w.comp_mut::<Brain>(id).expect("b").current_goal = Some(GoalKind::Raid);
}

fn set_fighter(w: &mut World, id: EntityId, fighting: f32, courage: f32) {
    w.comp_mut::<Skills>(id).expect("s").fighting = fighting;
    w.comp_mut::<Personality>(id).expect("p").courage = courage;
}

fn civilians(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<Brain>(id)).take(n).collect()
}

#[test]
fn test_brawl_with_no_defenders_sacks_the_hideout() {
    let mut w = world(49);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let h1 = w.hideout_of(g1).expect("h1");
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    w.comp_mut::<Building>(h1).expect("b").stock_food = 20;
    let door_tile = raid::rival_hideout_tile(&w, a).expect("tile");
    stage_raider(&mut w, a, door_tile);
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    assert_eq!(raid::brawl(&mut w, a), Some(Outcome::Sacked));
    assert_eq!(w.comp::<Gang>(g0).expect("g").treasury, 150);
    assert_eq!(w.comp::<Gang>(g1).expect("g").treasury, 0);
    assert!(w.comp::<Gang>(g1).expect("g").is_sacked(w.tick));
    assert!(w.comp::<Gang>(g1).expect("g").shocks.contains(&Shock::Sacked));
    assert_eq!(w.comp::<Building>(h1).expect("b").stock_food, 0);
    assert_eq!(w.comp::<Building>(w.hideout_of(g0).expect("h0")).expect("b").stock_food, 20);
    assert_eq!(w.comp::<Gang>(g0).expect("g").raid_at, None);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Sacked));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Raid && e.text.contains("Sacked")));
    assert_eq!(raid::brawl(&mut w, a), None, "a late raider finds it resolved");
    // The shock list is drained by the rethink; the loser keeps its grievance in retaliate_until.
    assert!(w.comp::<Gang>(g1).expect("g").retaliate_until.is_some());
}

#[test]
fn test_brawl_three_on_one_takes_the_treasury() {
    let mut w = world(50);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let civ = civilians(&w, 4);
    let (r1, r2, r3, d) = (civ[0], civ[1], civ[2], civ[3]);
    for r in [r1, r2, r3] {
        gang::enlist(&mut w, r, g0);
        set_fighter(&mut w, r, 0.9, 0.9);
    }
    temper(&mut w, r1);
    gang::enlist(&mut w, d, g1);
    set_fighter(&mut w, d, 0.1, 0.1);
    let h1 = w.hideout_of(g1).expect("h1");
    w.leave_building(d);
    w.enter_building(d, h1);
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    let door_tile = raid::rival_hideout_tile(&w, r1).expect("tile");
    for r in [r1, r2, r3] {
        stage_raider(&mut w, r, door_tile);
    }
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    let out = raid::brawl(&mut w, r1).expect("resolved");
    assert!(matches!(out, Outcome::Won | Outcome::Sacked), "{out:?}");
    assert!(w.comp::<Gang>(g0).expect("g").treasury >= 100, "half the prize at least");
    assert!(w.comp::<Gang>(g1).expect("g").treasury <= 50);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Assault), "every pairing is an Assault");
}

#[test]
fn test_brawl_one_on_three_loses_and_the_leader_retaliates() {
    let mut w = world(51);
    w.config.crime.fight_death_p = 0.0;
    let (g0, g1) = gangs(&w);
    let civ = civilians(&w, 4);
    let (r, d1, d2, d3) = (civ[0], civ[1], civ[2], civ[3]);
    gang::enlist(&mut w, r, g0);
    temper(&mut w, r);
    set_fighter(&mut w, r, 0.1, 0.5);
    let h1 = w.hideout_of(g1).expect("h1");
    for d in [d1, d2, d3] {
        gang::enlist(&mut w, d, g1);
        set_fighter(&mut w, d, 0.9, 0.9);
        w.leave_building(d);
        w.enter_building(d, h1);
    }
    w.comp_mut::<Gang>(g1).expect("g").treasury = 100;
    let door_tile = raid::rival_hideout_tile(&w, r).expect("tile");
    stage_raider(&mut w, r, door_tile);
    w.tick = 700;
    w.comp_mut::<Gang>(g0).expect("g").raid_at = Some(w.tick);
    assert_eq!(raid::brawl(&mut w, r), Some(Outcome::Lost));
    assert_eq!(w.comp::<Gang>(g1).expect("g").treasury, 100);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.order, Order::Retaliate, "a lost raid is a grudge");
    assert_eq!(g.raid_at, Some(1320), "the counter-raid musters tonight");
}

#[test]
fn test_raid_goal_opens_in_the_gather_window() {
    let mut w = world(52);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    w.tick = 600;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_at = Some(1320);
    }
    assert!(!raid::raid_pending(&w, a), "22:00 is twelve hours off");
    assert!(!raid::raid_done(&w, a));
    w.tick = 1200;
    assert!(raid::raid_pending(&w, a), "two hours off: muster");
    assert!(!raid::mustered(&w, a));
    w.tick = 1320;
    assert!(raid::mustered(&w, a));
    assert!(raid::depart(&mut w, a));
    assert_eq!(w.comp::<Gang>(g0).expect("g").last_raid_tick, Some(1320));
    w.comp_mut::<Gang>(g0).expect("g").raid_at = None;
    assert!(raid::raid_done(&w, a));
    assert!(!raid::depart(&mut w, a), "no muster to depart from");
}

/// Every guard far from the Jail door, out of any building.
fn clear_guards(w: &mut World) {
    for g in w.citizens().into_iter().filter(|&g| citysim::systems::law::is_guard(w, g)).collect::<Vec<_>>() {
        w.abort_plan(g);
        w.leave_building(g);
        w.comp_mut::<citysim::Position>(g).expect("p").tile = citysim::TilePos { x: 95, y: 0 };
    }
}

/// Jail `who` for `crime` (the leader's arrest makes them the boss).
fn jail(w: &mut World, who: EntityId, crime: citysim::Crime) {
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let until = w.tick + citysim::systems::law::sentence_ticks(w, crime);
    citysim::systems::law::sentence(w, who, crime, until, jail);
}

#[test]
fn test_breach_with_no_guards_frees_the_boss_first_and_reopens_the_warrant() {
    use citysim::Crime;
    let mut w = world(58);
    w.config.crime.fight_death_p = 0.0;
    let (g0, _) = gangs(&w);
    let civ = civilians(&w, 3);
    let (boss, grunt, raider) = (civ[0], civ[1], civ[2]);
    for &m in &[boss, grunt, raider] {
        gang::enlist(&mut w, m, g0);
    }
    w.comp_mut::<Personality>(boss).expect("p").loyalty = 1.0;
    gang::recompute_leader(&mut w, g0);
    assert_eq!(w.comp::<Gang>(g0).expect("g").leader, Some(boss));
    w.config.gangs.breakout_max_freed = 1;
    jail(&mut w, grunt, Crime::Theft);
    jail(&mut w, boss, Crime::Assault);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.boss, Some(boss), "the jailed leader is the boss");
    assert_eq!(g.leader, Some(raider));
    assert!(!citysim::systems::law::wanted(&w, boss), "the arrest resolved the warrant");

    clear_guards(&mut w);
    let door = raid::jail_tile(&w).expect("jail door");
    stage_raider(&mut w, raider, door);
    {
        let now = w.tick;
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::BreakOut;
        g.raid_at = Some(now);
    }
    assert_eq!(raid::target_tile(&w, raider), Some(door), "a breakout marches on the Jail");
    assert_eq!(raid::resolve(&mut w, raider), Some(Outcome::Won));
    assert!(!w.has::<citysim::Sentence>(boss), "the boss is out");
    assert!(w.has::<citysim::Sentence>(grunt), "one freed per breach: the boss first");
    assert!(citysim::systems::law::wanted(&w, boss), "the warrant reopens");
    assert!(w.comp::<citysim::Position>(boss).is_some_and(|p| p.building.is_none()), "at the Jail door");
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.boss, None);
    assert_eq!(g.leader, Some(boss), "and leads again");
    assert!(g.last_breakout_tick.is_some());
    // The rethink may already have mustered a raid (the rival here is empty
    // and rich); the breakout itself is over.
    assert!(!g.order.target_is_jail());
    assert!(w.events.iter().any(|e| e.kind == EventKind::Jailbreak && e.text.contains("broke 1 out")));
    assert!(w
        .comp::<citysim::Memory>(boss)
        .is_some_and(|m| m.entries.iter().any(|e| e.kind == citysim::MemoryKind::Escaped)));
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::BreakOut;
        g.raid_at = None;
    }
    assert_eq!(raid::resolve(&mut w, raider), None, "a late raider finds it resolved");
}

#[test]
fn test_breach_frees_the_longest_remaining_sentence_after_the_boss() {
    use citysim::Crime;
    let mut w = world(69);
    w.config.crime.fight_death_p = 0.0;
    let (g0, _) = gangs(&w);
    let civ = civilians(&w, 3);
    let (short, long, raider) = (civ[0], civ[1], civ[2]);
    for &m in &[short, long, raider] {
        gang::enlist(&mut w, m, g0);
    }
    let jail_b = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let now = w.tick;
    citysim::systems::law::sentence(&mut w, short, Crime::Theft, now + 500, jail_b);
    citysim::systems::law::sentence(&mut w, long, Crime::Theft, now + 5000, jail_b);
    w.config.gangs.breakout_max_freed = 1;
    w.comp_mut::<Gang>(g0).expect("g").boss = None;
    clear_guards(&mut w);
    let door = raid::jail_tile(&w).expect("jail door");
    stage_raider(&mut w, raider, door);
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::BreakOut;
        g.raid_at = Some(now);
    }
    assert_eq!(raid::resolve(&mut w, raider), Some(Outcome::Won));
    assert!(!w.has::<citysim::Sentence>(long), "the longest sentence is freed");
    assert!(w.has::<citysim::Sentence>(short));
}

#[test]
fn test_breach_with_nobody_inside_fizzles() {
    let mut w = world(70);
    let (g0, _) = gangs(&w);
    let civ = civilians(&w, 2);
    for &m in &civ {
        gang::enlist(&mut w, m, g0);
    }
    clear_guards(&mut w);
    let door = raid::jail_tile(&w).expect("jail door");
    stage_raider(&mut w, civ[0], door);
    let now = w.tick;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::BreakOut;
        g.raid_at = Some(now);
    }
    raid::resolve(&mut w, civ[0]);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Jailbreak), "no jailbreak");
    assert!(w.comp::<Gang>(g0).expect("g").last_breakout_tick.is_some(), "the cooldown still runs");
    assert!(w.law().is_some_and(|l| l.last_breakout_tick.is_none()), "and the law is not shocked");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Raid && e.text.contains("fizzled")));
}

#[test]
fn test_breach_against_three_guards_fails_and_shocks() {
    use citysim::Crime;
    let mut w = world(59);
    w.config.crime.fight_death_p = 0.0;
    let (g0, _) = gangs(&w);
    let civ = civilians(&w, 3);
    let (convict, raider, other) = (civ[0], civ[1], civ[2]);
    for &m in &[convict, raider, other] {
        gang::enlist(&mut w, m, g0);
    }
    jail(&mut w, convict, Crime::Assault);
    clear_guards(&mut w);
    let jail_b = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let guards: Vec<EntityId> =
        w.citizens().into_iter().filter(|&g| citysim::systems::law::is_guard(&w, g)).take(3).collect();
    for &g in &guards {
        set_fighter(&mut w, g, 0.9, 0.9);
        w.enter_building(g, jail_b);
    }
    set_fighter(&mut w, raider, 0.1, 0.5);
    let door = raid::jail_tile(&w).expect("jail door");
    stage_raider(&mut w, raider, door);
    w.tick = 700;
    {
        let now = w.tick;
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::BreakOut;
        g.raid_at = Some(now);
    }
    assert_eq!(raid::resolve(&mut w, raider), Some(Outcome::Lost));
    assert!(w.has::<citysim::Sentence>(convict), "nobody freed");
    let g = w.comp::<Gang>(g0).expect("g");
    assert!(g.last_breakout_tick.is_some(), "the cooldown runs either way");
    assert_ne!(g.order, Order::BreakOut, "the brain rethought at once");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Assault && e.text.contains("at the Precinct")));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Raid && e.text.contains("stormed the Precinct: Lost")));
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Jailbreak));
}

#[test]
fn test_gang_members_are_never_statistical() {
    let mut w = world(53);
    let (g0, _) = gangs(&w);
    let members = civilians(&w, 3);
    for &m in &members {
        gang::enlist(&mut w, m, g0);
    }
    w.tick = 60;
    citysim::systems::lod::run(&mut w);
    let stat = w
        .citizens()
        .into_iter()
        .filter(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == citysim::Lod::Statistical))
        .count();
    assert!(stat > 100, "most of the city is Statistical ({stat})");
    for m in members {
        assert_ne!(w.comp::<Brain>(m).expect("b").lod, citysim::Lod::Statistical);
    }
}

#[test]
fn test_night_watch_rotates_through_the_grunts() {
    let mut w = world(56);
    let (g0, _) = gangs(&w);
    let members = civilians(&w, 5);
    for (i, &m) in members.iter().enumerate() {
        w.tick = i as u64; // distinct joining ticks: the newest stand watch first
        gang::enlist(&mut w, m, g0);
    }
    w.tick = 1300;
    let watch = |w: &World| -> Vec<EntityId> { members.iter().copied().filter(|&m| gang::on_watch(w, m)).collect() };
    let tonight = watch(&w);
    assert_eq!(tonight.len(), 2, "min(night_watch, n / 2)");
    for &m in &tonight {
        assert_eq!(gang::holes_up_at(&w, m), w.hideout_of(g0));
    }
    assert_eq!(watch(&w).len(), 2);
    w.tick = 1300 + 300; // 03:40, the same night
    assert_eq!(watch(&w), tonight, "a watch lasts the night");
    w.tick = 1300 + 1440;
    let tomorrow = watch(&w);
    assert_eq!(tomorrow.len(), 2);
    assert_ne!(tomorrow, tonight, "the duty goes round");
    // A jailed member leaves the roster: the watch is drawn from the free.
    let mut jailed = world(54);
    let (j0, _) = gangs(&jailed);
    let crew = civilians(&jailed, 4);
    for (i, &m) in crew.iter().enumerate() {
        jailed.tick = i as u64;
        gang::enlist(&mut jailed, m, j0);
    }
    jailed.tick = 1300;
    jail(&mut jailed, crew[0], citysim::Crime::Theft);
    jail(&mut jailed, crew[1], citysim::Crime::Theft);
    jail(&mut jailed, crew[2], citysim::Crime::Theft);
    assert!(crew.iter().all(|&m| !gang::on_watch(&jailed, m)), "one free member keeps no watch");
    // A gang of one keeps no watch.
    let mut lone = world(57);
    let (g0, _) = gangs(&lone);
    let (a, _) = two_civilians(&lone);
    gang::enlist(&mut lone, a, g0);
    assert!(!gang::on_watch(&lone, a));
}

#[test]
fn test_lying_low_sleeps_and_idles_at_the_hideout() {
    use citysim::systems::plan;
    let mut w = world(55);
    let (g0, _) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let hideout = w.hideout_of(g0).expect("h0");
    let home = w.comp::<citysim::Household>(a).and_then(|h| h.home).expect("home");
    w.abort_plan(a);
    w.leave_building(a);
    w.comp_mut::<Needs>(a).expect("n").energy = 0.2;
    w.tick = 1300;
    let steps = |w: &mut World| -> Vec<ActionKind> {
        w.abort_plan(a);
        plan::plan_for(w, a, GoalKind::Sleep);
        w.comp::<Brain>(a)
            .and_then(|b| b.plan.as_ref())
            .map(|p| p.steps.iter().map(|s| s.action).collect())
            .unwrap_or_default()
    };
    assert_eq!(steps(&mut w), vec![ActionKind::GoTo(LocationKey::Home), ActionKind::Sleep], "expanding: home to bed");
    w.comp_mut::<Gang>(g0).expect("g").order = Order::LieLow;
    assert_eq!(steps(&mut w), vec![ActionKind::GoTo(LocationKey::Hideout), ActionKind::Sleep], "lying low: hole up");
    assert_eq!(gang::holes_up_at(&w, a), Some(hideout));
    let idle = citysim::exec::routine::idle_plan(&w, a).expect("idle plan");
    assert_eq!(
        idle.steps.iter().map(|s| s.action).collect::<Vec<_>>(),
        vec![ActionKind::GoTo(LocationKey::Hideout), ActionKind::Rest]
    );
    // A full Hideout sends them home after all.
    let filler: Vec<EntityId> = civilians(&w, 13).into_iter().filter(|&c| c != a).take(12).collect();
    for c in filler {
        w.leave_building(c);
        w.enter_building(c, hideout);
    }
    assert!(w.comp::<Building>(hideout).expect("b").is_full());
    assert_eq!(gang::holes_up_at(&w, a), None);
    assert_eq!(steps(&mut w), vec![ActionKind::GoTo(LocationKey::Home), ActionKind::Sleep], "full: home after all");
    let _ = home;
}

#[test]
fn test_contest_keeps_working_a_home_after_the_first_blow() {
    let mut w = world(54);
    let (g0, g1) = gangs(&w);
    let (a, b) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    gang::enlist(&mut w, b, g1);
    temper(&mut w, a);
    let homes = [a, b].map(|x| w.comp::<citysim::Household>(x).and_then(|h| h.home));
    // The only rival-held Home: g1 holds it outright.
    let home = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .copied()
        .find(|&h| !homes.contains(&Some(h)) && w.comp::<Building>(h).is_some_and(|bd| bd.occupants.len() >= 2))
        .expect("an occupied home");
    for _ in 0..3 {
        gang::extort(&mut w, b, home);
    }
    assert_eq!(gang::holder_of(&w, home), Some(g1));
    w.comp_mut::<Gang>(g0).expect("g").order = Order::Contest;
    // Guards may stand near it; the target picker is what is under test, so clear them.
    for g in w.citizens().into_iter().filter(|&g| citysim::systems::law::is_guard(&w, g)).collect::<Vec<_>>() {
        w.comp_mut::<citysim::Position>(g).expect("p").tile = citysim::TilePos { x: 95, y: 0 };
    }
    assert_eq!(gang::gang_work_target(&w, a), Some((home, Some(Order::Contest))));
    gang::extort(&mut w, a, home);
    assert_eq!(w.comp::<Building>(home).expect("b").claim, Some(Claim { gang: g0, count: 1 }));
    assert_eq!(gang::gang_work_target(&w, a), Some((home, Some(Order::Contest))), "still the rival's until it flips");
    gang::extort(&mut w, a, home);
    gang::extort(&mut w, a, home);
    assert_eq!(gang::holder_of(&w, home), Some(g0));
    assert!(gang::gang_work_target(&w, a).is_none(), "nothing of the rival's left to contest");
}
