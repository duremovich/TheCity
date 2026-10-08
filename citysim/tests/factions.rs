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
        // M14 V34: the Retaliate target's cover, here the rival's (no hack grudge).
        retaliate_cover: 0.0,
        jail_cover: 0.0,
        derelicts: 0,
        districts_held: 0,
        open_districts: 0,
        corp_prize: None,
        corp_cover: 0.0,
        corp_guards: 0,
        corp_raids: false,
        clinic_exists: false,
        harvest_target: None,
        harvest_cover: 0.0,
        lawfulness: 0.5,
        treasury_x: 0.0,
        runner: None,
        virt_ev: 0.0,
        virt_p: 0.0,
        hacked: false,
        virt_grudge: false,
        fear: None,
        vendetta: None,
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
    let i = OrderInputs { grudge: true, own: 3, rival: 8, ..inputs() };
    assert_eq!(best(&i), Order::Retaliate);
    // M12 phase 4: below `raid_min_members` (3) there is no crew to send.
    let alone = OrderInputs { own: 2, ..i.clone() };
    assert_ne!(best(&alone), Order::Retaliate);
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
        citysim::OrderScore { order: Order::Contest, score: 0.50, considerations: vec![], corp_target: false },
        citysim::OrderScore { order: Order::Expand, score: 0.45, considerations: vec![], corp_target: false },
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

/// L2 (L22): with `[lod] budget` on, each gang holds class 2 for its first
/// `gang_quota` members only (the leader always), and the rotation hands
/// the slots round from day to day.
#[test]
fn test_gang_quota_holds_and_rotates() {
    let mut cfg = Config::load().v1_profile();
    cfg.lod.budget = true;
    cfg.living.enabled = true; // the master switch (`v1_profile` turns L2 off)
    cfg.gangs.max_members = 200;
    let mut w = World::new(53, cfg);
    let (g0, _) = gangs(&w);
    let members = civilians(&w, 60);
    assert_eq!(members.len(), 60);
    for &m in &members {
        gang::enlist(&mut w, m, g0);
    }
    let quota = w.config.lod.gang_quota;
    let leader = w.comp::<Gang>(g0).and_then(|g| g.leader).expect("a leader");
    w.tick = 600;
    citysim::systems::lod::run(&mut w);
    let class2 = |w: &World| -> Vec<EntityId> {
        members.iter().copied().filter(|&m| citysim::systems::lod::rank_class(w, m) == 2).collect()
    };
    let today = class2(&w);
    assert!(today.len() <= quota, "{} class-2 members over the quota {quota}", today.len());
    assert_eq!(today.len(), quota);
    assert!(today.contains(&leader), "the leader always ranks");
    let stat =
        members.iter().filter(|&&m| w.comp::<Brain>(m).is_some_and(|b| b.lod == citysim::Lod::Statistical)).count();
    assert!(stat > 0, "members over the quota may be Statistical");
    w.tick = 600 + citysim::TICKS_PER_DAY;
    let tomorrow = class2(&w);
    assert_eq!(tomorrow.len(), quota);
    assert!(tomorrow.contains(&leader));
    assert_ne!(today, tomorrow, "the rotation hands the slots round");
}

/// M10 D20 as before L2: without the budget gang members are never Statistical.
#[test]
fn test_gang_members_are_never_statistical_without_budget() {
    let mut w = world(53);
    assert!(!w.config.lod.budget);
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

// ---------------------------------------------------------------------------
// M12 phase 4: gangs in districts (docs/M12_DISTRICTS.md § 5, plan D36-D40)
// ---------------------------------------------------------------------------

use citysim::{Controller, DistrictId, Position, TilePos, TICKS_PER_DAY};

/// The 400-resident v2 city (districts, corps, Lots) with today's aggregates.
fn v2(seed: u64) -> World {
    let mut w = World::new(seed, Config::load().scaled_to(400));
    w.config.crime.fight_death_p = 0.0;
    w.config.riots.p_crossfire = 0.0;
    citysim::systems::districts::daily(&mut w);
    w
}

/// Hold `homes` for `g` (claims at three, territory sorted).
fn hold(w: &mut World, g: EntityId, homes: &[EntityId]) {
    for &h in homes {
        w.comp_mut::<Building>(h).expect("b").claim = Some(Claim { gang: g, count: gang::CLAIM_HELD });
    }
    let mut t = homes.to_vec();
    t.sort_unstable();
    w.comp_mut::<Gang>(g).expect("g").territory = t;
}

/// Homes of district `d`, nearest `from` first.
fn homes_by_distance(w: &World, d: u8, from: TilePos) -> Vec<(u32, EntityId)> {
    let mut v: Vec<(u32, EntityId)> = w.districts[usize::from(d)]
        .homes
        .iter()
        .filter_map(|&h| w.comp::<Building>(h).map(|b| (b.door.manhattan(from), h)))
        .collect();
    v.sort();
    v
}

#[test]
fn test_muster_point_is_held_home_nearest_target() {
    let mut w = v2(60);
    // The rule at the spec's radius (the assets calibrate it wider).
    w.config.gangs.muster_near_tiles = 48;
    let (g0, g1) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let target = w.hideout_of(g1).and_then(|h| w.comp::<Building>(h)).map(|b| b.door).expect("rival door");
    let d = w.district_of(target).0;
    let homes = homes_by_distance(&w, d, target);
    let near = homes.iter().find(|&&(dist, _)| (20..=40).contains(&dist)).map(|&(_, h)| h).expect("a Home at 20-40");
    let far = homes.iter().find(|&&(dist, _)| (49..=90).contains(&dist)).map(|&(_, h)| h).expect("a Home past 48");
    hold(&mut w, g0, &[near, far]);
    let now = w.tick;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_at = Some(now + 60);
    }
    let door = |w: &World, h: EntityId| w.comp::<Building>(h).map(|b| w.outside_door(b)).expect("door");
    assert_eq!(raid::muster_point(&w, a), Some(raid::MusterAt::Door(door(&w, near))), "the held Home near the target");
    // The plan reaches the muster point: it is a known distance, and the
    // Muster precondition is met standing on that tile.
    let ctx = citysim::PlanCtx::build(&w, a, None);
    assert!(ctx.dist.contains_key(&citysim::LocationKey::MusterPoint));
    w.leave_building(a);
    let tile = door(&w, near);
    let p = w.comp_mut::<Position>(a).expect("pos");
    p.tile = tile;
    p.building = None;
    w.comp_mut::<Brain>(a).expect("b").current_goal = Some(GoalKind::Raid);
    let ws = citysim::WorldState::observe(&w, a, None);
    assert_eq!(ws.at, citysim::LocationKey::MusterPoint);
    // Only a Home past muster_near_tiles: the Hideout, as in M8.
    hold(&mut w, g0, &[far]);
    let hq = w.hideout_of(g0).expect("hq");
    assert_eq!(raid::muster_point(&w, a), Some(raid::MusterAt::Inside(hq)));
    // muster_near_tiles = 0 is the M8 muster.
    hold(&mut w, g0, &[near]);
    w.config.gangs.muster_near_tiles = 0;
    assert_eq!(raid::muster_point(&w, a), Some(raid::MusterAt::Inside(hq)));
}

#[test]
fn test_raid_into_cover_is_not_chosen_or_departed() {
    let cfg = Config::load().gangs;
    // The corp variant obeys its own cover.
    let base = OrderInputs {
        frontier: 0,
        rival_exists: false,
        prize: 0,
        own: 10,
        corp_prize: Some((EntityId::none(), 500)),
        corp_raids: true,
        clinic_exists: false,
        harvest_target: None,
        harvest_cover: 0.0,
        lawfulness: 0.5,
        treasury_x: 0.0,
        runner: None,
        virt_ev: 0.0,
        virt_p: 0.0,
        hacked: false,
        virt_grudge: false,
        fear: None,
        vendetta: None,
        hoard: 0.5,
        hoard_tilt: 0.2,
        courage: 0.8,
        ..inputs()
    };
    let raid = |i: &OrderInputs| faction::score_orders(i, &cfg).into_iter().find(|s| s.order == Order::Raid);
    let open = raid(&base).expect("a corp Raid under no cover");
    assert!(faction::is_corp_raid(&open));
    assert!(raid(&OrderInputs { corp_cover: 1.0, ..base.clone() }).is_none(), "full cover gates it");
    // The rival variant still scores when it beats the corp one.
    let both = OrderInputs { rival_exists: true, prize: 400, rival: 2, ..base.clone() };
    assert!(raid(&both).is_some());
    // The live gang: a Crackdown on the raider where the rival's Hideout
    // stands closes the gate at scoring and at the muster's departure.
    let mut w = v2(61);
    let (g0, g1) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let d = w.district_of_building(w.hideout_of(g1).expect("hq"));
    w.district_mut(d).stance = citysim::Stance::Crackdown(g0);
    let i = faction::gather_inputs(&w, g0).expect("inputs");
    assert_eq!(i.target_cover, 1.0);
    assert!(faction::score_orders(&i, &w.config.gangs)
        .iter()
        .all(|s| s.order != Order::Raid || faction::is_corp_raid(s)));
    let now = w.tick;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_at = Some(now);
    }
    assert!(!raid::depart(&mut w, a), "no raid departs into a district cracking down on the raider");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Raid && e.text.contains("called off")));
    w.district_mut(d).stance = citysim::Stance::Patrol;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_at = Some(now);
    }
    assert!(raid::depart(&mut w, a));
}

#[test]
fn test_corp_building_raid_takes_documented_prize_against_private_guards() {
    use citysim::{Corp, CorpShock, Inventory};
    let mut w = v2(62);
    // The posted guards have their own test; here only the one at the door.
    w.config.gangs.corp_raid_posted = 0;
    let (g0, _) = gangs(&w);
    // A corp Market, its owner rich, one private guard on contract at its door.
    let market = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| w.corp_of_building(m).is_some())
        .expect("a corp Market");
    let corp = w.corp_of_building(market).expect("corp");
    w.comp_mut::<Corp>(corp).expect("c").treasury = 20_000;
    let door = w.comp::<Building>(market).map(|b| w.outside_door(b)).expect("door");
    let far = TilePos { x: 250, y: 2 };
    for g in w.guards().to_vec() {
        w.abort_plan(g);
        w.leave_building(g);
        let p = w.comp_mut::<Position>(g).expect("pos");
        p.tile = far;
        p.building = None;
    }
    if let Some(l) = w.law_mut() {
        l.beats.clear();
    }
    let office = w.buildings_of_kind(BuildingKind::SecurityOffice)[0];
    let pg = civilians(&w, 1)[0];
    citysim::systems::demography::hire(&mut w, pg, office, citysim::Role::Guard);
    assert!(citysim::systems::law::is_private_guard(&w, pg));
    w.leave_building(pg);
    let security = w.corp_of_building(office).expect("a security corp");
    w.comp_mut::<Building>(market).expect("b").secured_by = Some(security);
    assert!(raid::private_guards_of(&w, market).contains(&pg));
    {
        let p = w.comp_mut::<Position>(pg).expect("pos");
        p.tile = door;
    }
    set_fighter(&mut w, pg, 0.0, 0.0);
    for s in citysim::systems::ownership::staff_at(&w, market) {
        w.leave_building(s);
    }
    w.comp_mut::<Building>(market).expect("b").stock_food = 100;
    let raiders = civilians(&w, 3);
    for &r in &raiders {
        gang::enlist(&mut w, r, g0);
        set_fighter(&mut w, r, 1.0, 1.0);
        stage_raider(&mut w, r, door);
    }
    temper(&mut w, raiders[0]);
    let food_before: u32 = raiders.iter().map(|&r| w.comp::<Inventory>(r).map_or(0, |i| i.food)).sum();
    let treasury = w.comp::<Gang>(g0).expect("g").treasury;
    let now = w.tick;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_target = Some(market);
        g.raid_at = Some(now);
    }
    assert_eq!(raid::corp_prize_value(&w, corp), 500, "min(5 % of 20,000, 500)");
    let out = raid::resolve(&mut w, raiders[0]).expect("resolved");
    assert_eq!(out, Outcome::Won);
    assert_eq!(w.comp::<Gang>(g0).expect("g").treasury, treasury + 500);
    assert_eq!(w.comp::<Corp>(corp).expect("c").treasury, 19_500);
    // Fix pass: `corp_raid_loot_frac` (0.25) of the stock, not all of it.
    assert_eq!(w.comp::<Building>(market).expect("b").stock_food, 75);
    let food_after: u32 = raiders.iter().map(|&r| w.comp::<Inventory>(r).map_or(0, |i| i.food)).sum();
    assert_eq!(food_after, food_before + 25, "a quarter of the stock goes to the raiders");
    assert_eq!(w.comp::<Corp>(corp).expect("c").raided_at, Some(now), "the corp remembers the raid");
    let shocks = &w.comp::<Corp>(corp).expect("c").shocks;
    assert!(shocks.contains(&CorpShock::Robbed(500)) && shocks.contains(&CorpShock::Raided), "{shocks:?}");
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Raid).expect("a Raid event");
    assert!(e.text.contains(" raided ") && e.text.contains("3 raiders vs 1 defenders"), "{}", e.text);
    assert!(e.text.contains("500 coins and 25 food taken"), "item 41: the real amount: {}", e.text);
    let g = w.comp::<Gang>(g0).expect("g");
    assert!(g.raid_at.is_none() && g.raid_target.is_none());
}

/// Fix pass (item 28): a contracted corp building's private guards are
/// posted at its door when a raid comes (on shift, or any shift while the
/// owner holds Secure), up to `corp_raid_posted`, and a crew that loses a
/// third of its pairings breaks: the raid is lost.
#[test]
fn test_corp_raid_meets_posted_guards_and_the_crew_breaks() {
    use citysim::{Corp, CorpOrder};
    let mut w = v2(62);
    let (g0, _) = gangs(&w);
    let market = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find(|&m| w.corp_of_building(m).is_some())
        .expect("a corp Market");
    let corp = w.corp_of_building(market).expect("corp");
    let door = w.comp::<Building>(market).map(|b| w.outside_door(b)).expect("door");
    // Across the map from the door (out of `answer_radius` on x).
    let far = TilePos { x: if door.x < 128 { 250 } else { 2 }, y: 2 };
    for g in w.guards().to_vec() {
        w.abort_plan(g);
        w.leave_building(g);
        let p = w.comp_mut::<Position>(g).expect("pos");
        p.tile = far;
        p.building = None;
    }
    if let Some(l) = w.law_mut() {
        l.beats.clear();
    }
    for s in citysim::systems::ownership::staff_at(&w, market) {
        w.leave_building(s);
    }
    // The owner holds Secure and buys a contract: every guard of the seller
    // may be called in, whatever the shift.
    w.comp_mut::<Corp>(corp).expect("c").order = CorpOrder::Secure;
    let office = w.buildings_of_kind(BuildingKind::SecurityOffice)[0];
    let security = w.corp_of_building(office).expect("a security corp");
    w.comp_mut::<Building>(market).expect("b").secured_by = Some(security);
    // M12 review: four of the seller's guards are up within answer_radius
    // of the door; a fifth is across the map and a sixth near but off shift
    // and asleep: neither is called in (no teleport from a bed or the far
    // side).
    let near = TilePos { x: if door.x >= 10 { door.x - 10 } else { door.x + 10 }, y: door.y };
    assert!(u32::from(far.x.abs_diff(door.x)) > w.config.crime.answer_radius, "the far tile is out of reach");
    let hired = civilians(&w, 6);
    for (i, &pg) in hired.iter().enumerate() {
        citysim::systems::demography::hire(&mut w, pg, office, citysim::Role::Guard);
        w.leave_building(pg);
        let p = w.comp_mut::<Position>(pg).expect("pos");
        p.tile = if i == 4 { far } else { near };
        p.building = None;
    }
    w.comp_mut::<Brain>(hired[5]).expect("brain").current_goal = Some(citysim::GoalKind::Sleep);
    let tod = w.tick_of_day();
    let off = if tod < 720 { (1000, 1100) } else { (100, 200) };
    w.comp_mut::<citysim::Job>(hired[5]).expect("job").shifts = vec![off];
    assert!(!w.comp::<citysim::Job>(hired[5]).expect("job").on_shift(tod));
    let roster = raid::private_guards_of(&w, market);
    assert!(roster.len() >= 6, "the seller's guards: {}", roster.len());
    w.config.gangs.corp_raid_posted = 6;
    let reachable = raid::posted_guards(&w, market);
    assert_eq!(reachable.len(), 4, "only the four awake guards in reach: {reachable:?}");
    assert!(!reachable.contains(&hired[4]) && !reachable.contains(&hired[5]));
    w.config.gangs.corp_raid_posted = 3;
    let posted = raid::posted_guards(&w, market);
    assert_eq!(posted.len(), 3, "capped at corp_raid_posted");
    for &g in &posted {
        set_fighter(&mut w, g, 1.0, 1.0);
    }
    let raiders = civilians(&w, 4);
    for &r in &raiders {
        gang::enlist(&mut w, r, g0);
        set_fighter(&mut w, r, 0.0, 0.0);
        stage_raider(&mut w, r, door);
    }
    temper(&mut w, raiders[0]);
    let now = w.tick;
    {
        let g = w.comp_mut::<Gang>(g0).expect("g");
        g.order = Order::Raid;
        g.raid_target = Some(market);
        g.raid_at = Some(now);
    }
    let out = raid::resolve(&mut w, raiders[0]).expect("resolved");
    assert_eq!(out, Outcome::Lost);
    for &g in &posted {
        let at = w.comp::<Position>(g).expect("pos").tile;
        assert!(at.manhattan(door) <= 2, "posted guard stood at the door: {at:?}");
    }
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Raid).expect("a Raid event");
    assert!(e.text.contains("4 raiders vs 3 defenders") && e.text.contains("Lost"), "{}", e.text);
    // ceil(4 x 0.34) = 2 pairings lost and the crew scatters: two raiders
    // never fought (no Assault event names them).
    let fought = |r: EntityId| w.events.iter().any(|e| e.kind == EventKind::Assault && e.actors.first() == Some(&r));
    assert_eq!(raiders.iter().filter(|&&r| fought(r)).count(), 2, "the crew broke after two losses");
}

/// A gang of `n` in g0 for the split tests: the old leader (loyalty 0.95),
/// the new leader L (0.3, fighting 0.5), the lieutenant (0.25, fighting
/// 0.8), the rest 0.1. Returns (old, leader, lieutenant, all).
fn split_gang(w: &mut World, g0: EntityId, n: usize) -> (EntityId, EntityId, EntityId, Vec<EntityId>) {
    let crew = civilians(w, n);
    for &m in &crew {
        gang::enlist(w, m, g0);
        let p = w.comp_mut::<Personality>(m).expect("p");
        p.loyalty = 0.1;
    }
    let (old, leader, lt) = (crew[0], crew[1], crew[2]);
    w.comp_mut::<Personality>(old).expect("p").loyalty = 0.95;
    w.comp_mut::<Personality>(leader).expect("p").loyalty = 0.3;
    w.comp_mut::<Personality>(lt).expect("p").loyalty = 0.25;
    set_fighter(w, leader, 0.5, 0.5);
    set_fighter(w, lt, 0.8, 0.5);
    gang::recompute_leader(w, g0);
    assert_eq!(w.comp::<Gang>(g0).expect("g").leader, Some(old));
    (old, leader, lt, crew)
}

#[test]
fn test_decapitation_splits_with_strong_lieutenant_and_two_districts() {
    let mut w = v2(63);
    w.config.gangs.split_base = 1.0;
    let (g0, _) = gangs(&w);
    let (old, leader, lt, _) = split_gang(&mut w, g0, 8);
    // Homes in Sump West (5, the leader's) and Sump Central (6).
    let west: Vec<EntityId> = w.districts[5].homes.iter().copied().take(3).collect();
    let central: Vec<EntityId> = w.districts[6].homes.iter().copied().take(4).collect();
    let mut all = west.clone();
    all.extend(&central);
    hold(&mut w, g0, &all);
    w.comp_mut::<citysim::Household>(leader).expect("h").home = Some(west[0]);
    // One held district: no split.
    hold(&mut w, g0, &west);
    assert!(gang::split(&mut w, g0, None, false).is_err());
    hold(&mut w, g0, &all);
    // The leader falls: a new leader, and a split check is due.
    w.kill_by(old, citysim::DeathCause::Violence, None);
    let g = w.comp::<Gang>(g0).expect("g");
    assert_eq!(g.leader, Some(leader));
    assert_eq!(g.split_check, Some(old));
    w.comp_mut::<Gang>(g0).expect("g").split_check = None;
    // Fix pass (item 35): the old boss living in the splinter's district stays.
    let crew = w.comp::<Gang>(g0).expect("g").members.clone();
    let boss = crew.iter().copied().find(|&m| m != leader && m != lt).expect("a third member");
    w.comp_mut::<citysim::Household>(boss).expect("h").home = Some(central[1]);
    w.comp_mut::<Gang>(g0).expect("g").boss = Some(boss);
    // Fix pass (item 40): at the cap, but one gang is an emptied ghost.
    let gangs_before = w.gang_list().len();
    w.config.gangs.max_gangs = gangs_before;
    let ghost = w.gang_list().iter().copied().find(|&g| g != g0).expect("another gang");
    {
        let gg = w.comp_mut::<Gang>(ghost).expect("ghost");
        gg.members.clear();
        gg.emptied = true;
    }
    let splinter = gang::split(&mut w, g0, Some(old), false).expect("a split");
    // M14 V4: the split marks the plane dirty; the next tick's relink gives
    // the new Hideout a live node owned by the splinter.
    citysim::systems::virt::run(&mut w);
    let hideout = w.comp::<Gang>(splinter).expect("splinter").hideout;
    let n = citysim::systems::virt::node_of_building(&w, hideout).expect("the splinter's Hideout has a node");
    assert!(w.virt.nodes[n.index()].alive);
    assert_eq!(citysim::systems::virt::owner_of(&w, n), Some(splinter));
    assert_eq!(w.gang_list().len(), gangs_before + 1);
    assert_eq!(w.gang_of(boss), Some(g0), "the jailed boss stays with the gang it ran");
    let sg = w.comp::<Gang>(splinter).expect("splinter");
    assert_eq!(w.district_of_building(sg.hideout), DistrictId(6), "the splinter's Hideout is in Sump Central");
    assert_eq!(w.comp::<Building>(sg.hideout).map(|b| b.kind), Some(BuildingKind::Hideout));
    assert!(sg.members.contains(&lt), "the lieutenant leads it");
    assert_eq!(sg.leader, Some(lt));
    assert_eq!(sg.split_from, Some(g0));
    assert!(!sg.territory.is_empty() && sg.territory.iter().all(|&h| w.district_of_building(h) == DistrictId(6)));
    for &h in &sg.territory {
        assert_eq!(w.comp::<Building>(h).and_then(|b| b.claim).map(|c| c.gang), Some(splinter));
    }
    let og = w.comp::<Gang>(g0).expect("old");
    assert!(og.territory.iter().all(|&h| w.district_of_building(h) == DistrictId(5)), "the rest keep Sump West");
    assert!(!og.members.contains(&lt) && og.members.contains(&leader));
    assert_eq!(w.gang_of(lt), Some(splinter));
    assert!(w.edge(lt, leader).is_some_and(|e| e.kind == citysim::RelKind::Enemy), "the halves are enemies");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Split && e.text.contains("Rust Saints")));
    // A district with neither a squat nor a Lot (Sump East): no split.
    let mut w2 = v2(64);
    w2.config.gangs.split_base = 1.0;
    let (h0, _) = gangs(&w2);
    let (_, leader2, _, _) = split_gang(&mut w2, h0, 8);
    let west2: Vec<EntityId> = w2.districts[5].homes.iter().copied().take(2).collect();
    let east2: Vec<EntityId> = w2.districts[7].homes.iter().copied().take(4).collect();
    let mut all2 = west2.clone();
    all2.extend(&east2);
    hold(&mut w2, h0, &all2);
    w2.comp_mut::<citysim::Household>(leader2).expect("h").home = Some(west2[0]);
    let err = gang::split(&mut w2, h0, None, false).expect_err("no Hideout site");
    assert!(err.contains("no squat or Lot"), "{err}");
}

#[test]
fn test_empty_gang_cannot_reform_in_held_district_or_before_reform_days() {
    let mut w = v2(65);
    let (g0, g1) = gangs(&w);
    // A fresh, never-manned gang is open (the city's first recruits).
    assert!(gang::may_reform(&w, g0));
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    gang::leave(&mut w, a, "test");
    assert!(w.comp::<Gang>(g0).expect("g").emptied);
    assert!(!gang::may_reform(&w, g0), "not before reform_days");
    w.tick += w.config.gangs.reform_days * TICKS_PER_DAY;
    let d = w.district_of_building(w.hideout_of(g0).expect("hq"));
    w.district_mut(d).coverage = 0.5;
    w.district_mut(d).control = Controller::Gang(g1);
    assert!(!gang::may_reform(&w, g0), "not in a district another gang holds");
    w.district_mut(d).control = Controller::Contested;
    w.district_mut(d).coverage = 1.5;
    assert!(!gang::may_reform(&w, g0), "not under the law's eye");
    w.district_mut(d).coverage = 0.5;
    assert!(gang::may_reform(&w, g0));
    // A recruit clears the flag.
    gang::enlist(&mut w, a, g0);
    assert!(!w.comp::<Gang>(g0).expect("g").emptied);
}

#[test]
fn test_empty_gang_loses_claims_after_three_days() {
    let mut w = v2(66);
    let (g0, _) = gangs(&w);
    let homes: Vec<EntityId> = w.districts[5].homes.iter().copied().take(3).collect();
    hold(&mut w, g0, &homes);
    let start = w.tick;
    w.comp_mut::<Gang>(g0).expect("g").empty_since = Some(start);
    w.tick = start + 2 * TICKS_PER_DAY;
    gang::clear_claims_if_empty(&mut w, g0);
    assert_eq!(w.comp::<Gang>(g0).expect("g").territory.len(), 3, "two days: still held");
    w.tick = start + 3 * TICKS_PER_DAY;
    gang::clear_claims_if_empty(&mut w, g0);
    assert!(w.comp::<Gang>(g0).expect("g").territory.is_empty());
    for h in homes {
        assert_eq!(w.comp::<Building>(h).and_then(|b| b.claim), None);
    }
    // Off at 0 (M11).
    let mut w2 = v2(66);
    w2.config.gangs.empty_claims_days = 0;
    let (h0, _) = gangs(&w2);
    let homes2: Vec<EntityId> = w2.districts[5].homes.iter().copied().take(3).collect();
    hold(&mut w2, h0, &homes2);
    w2.tick += 40 * TICKS_PER_DAY;
    gang::clear_claims_if_empty(&mut w2, h0);
    assert_eq!(w2.comp::<Gang>(h0).expect("g").territory.len(), 3);
}

#[test]
fn test_expand_frontier_reads_held_and_open_districts() {
    let mut w = v2(67);
    let (g0, g1) = gangs(&w);
    let (a, _) = two_civilians(&w);
    gang::enlist(&mut w, a, g0);
    temper(&mut w, a);
    let b = w.citizens().into_iter().find(|&x| x != a && w.gang_of(x).is_none() && w.has::<Brain>(x)).expect("b");
    gang::enlist(&mut w, b, g1);
    for d in w.districts.iter_mut() {
        d.control = Controller::City;
    }
    w.district_mut(DistrictId(5)).control = Controller::Gang(g0);
    let i = faction::gather_inputs(&w, g0).expect("inputs");
    assert_eq!((i.districts_held, i.open_districts), (1, 0));
    let held_total = i.frontier_total;
    assert!(held_total > 0 && held_total <= w.districts[5].homes.len());
    // A district whose gang has nobody fit is open ground.
    w.district_mut(DistrictId(6)).control = Controller::Gang(g1);
    w.insert(b, citysim::Sentence { until_tick: w.tick + TICKS_PER_DAY, crime: citysim::Crime::Theft });
    let i = faction::gather_inputs(&w, g0).expect("inputs");
    assert_eq!(i.open_districts, 1);
    assert!(i.frontier_total > held_total, "Sump Central joins the frontier");
}
