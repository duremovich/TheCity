//! Save/load round trip.

use citysim::{save, BuildingKind, Config, EntityId, PlayerCommand, RelKind, World, TICKS_PER_DAY};

/// L2 (L29): a dry Scavenge streak is saved (the spec's `#[serde(skip)]`
/// would make a loaded world diverge mid-streak).
#[test]
fn test_scavenge_dry_survives_save_load() {
    let mut cfg = Config::load().v1_profile();
    cfg.lod.budget = true;
    cfg.living.enabled = true; // the master switch (`v1_profile` turns L2 off)
    let mut w = World::new(42, cfg);
    w.run_ticks(600);
    let a = w.citizens()[3];
    w.comp_mut::<citysim::Brain>(a).expect("brain").scavenge_dry = 2;
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(back.comp::<citysim::Brain>(a).expect("brain").scavenge_dry, 2);
    assert_eq!(save::to_ron(&back), text);
    w.run_ticks(600);
    back.run_ticks(600);
    assert_eq!(blake3::hash(save::to_ron(&w).as_bytes()), blake3::hash(save::to_ron(&back).as_bytes()));
}

#[test]
fn test_save_round_trips_commands_and_events() {
    let mut w = World::new(5, Config::load().v1_profile());
    w.push_command(PlayerCommand::ReleaseReserve { amount: 100 });
    w.push_command(PlayerCommand::SetTaxRate(0.2));
    w.run_ticks(10);
    assert_eq!(w.command_log.len(), 2);
    let player_actions = w.events.iter().filter(|e| e.kind == citysim::EventKind::PlayerAction).count();
    assert_eq!(player_actions, 2);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    // 600 + 100 released, minus the day-0 1% spoilage, minus the first ten ticks of shopping
    assert!(w.comp::<citysim::Building>(market).expect("market").stock_food <= 693);

    let text = save::to_ron(&w);
    let back = save::from_ron(&text).expect("load");
    assert_eq!(back.command_log, w.command_log);
    assert_eq!(back.events, w.events);
    assert_eq!(back.levers, w.levers);
    assert_eq!(save::to_ron(&back), text);
}

#[test]
fn test_save_file_helpers() {
    let dir = std::env::temp_dir().join(format!("citysim-save-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut w = World::new(9, Config::load().v1_profile());
    w.run_ticks(5);
    let p5 = save::save_path(&dir, 9, w.tick);
    save::save_to_file(&w, &p5).expect("save");
    w.run_ticks(5);
    let p10 = save::save_path(&dir, 9, w.tick);
    save::save_to_file(&w, &p10).expect("save");
    assert_eq!(save::newest_save(&dir, 9), Some(p10.clone()));
    assert_eq!(save::newest_save(&dir, 8), None);
    let back = save::load_from_file(&p10).expect("load");
    assert_eq!(back.tick, 10);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A current save is not touched by the migration: an in-progress claim survives.
#[test]
fn test_current_save_keeps_live_claims() {
    let mut w = World::new(12, Config::load().v1_profile());
    let gangs = w.gangs();
    let home = w.buildings_by_kind[&BuildingKind::Home][0];
    w.comp_mut::<citysim::Gang>(gangs[0]).expect("gang").territory = vec![home];
    w.comp_mut::<citysim::Building>(home).expect("home").claim = Some(citysim::Claim { gang: gangs[1], count: 2 });
    let back = save::from_ron(&save::to_ron(&w)).expect("load");
    assert_eq!(
        back.comp::<citysim::Building>(home).expect("home").claim,
        Some(citysim::Claim { gang: gangs[1], count: 2 })
    );
}

/// Every field of every edge, floats as bits, in key order.
type EdgeBits = ((EntityId, EntityId), (u32, u32, i32, RelKind, u64, Option<u64>, Option<u64>));

fn edge_bits(w: &World) -> Vec<EdgeBits> {
    w.edges
        .iter()
        .map(|(&k, e)| {
            let f = (
                e.affinity.to_bits(),
                e.trust.to_bits(),
                e.debt,
                e.kind,
                e.last_interaction,
                e.last_birth_tick,
                e.debt_since,
            );
            (k, f)
        })
        .collect()
}

/// M12: the packed edge string round-trips bit-exactly, and saving the
/// loaded world again gives the same bytes.
#[test]
fn test_save_edges_round_trip_lossless() {
    let mut w = World::new(42, Config::load().v1_profile());
    w.run_ticks(10 * TICKS_PER_DAY);
    assert!(w.edges.len() > 100, "a city ten days in knows people");
    assert!(w.edges.values().any(|e| e.trust != 0.3), "some trust has moved");
    let text = save::to_ron(&w);
    assert!(text.contains("edges:\"e1:"), "edges are saved packed");
    let back = save::from_ron(&text).expect("load");
    assert_eq!(edge_bits(&back), edge_bits(&w));
    assert_eq!(save::to_ron(&back), text);
}

/// M15 (plan Risks, determinism): with the word on, a save taken mid-day
/// (pools, heard stores and the last midnight's reputation in it) loads and
/// runs on bit for bit, and two runs of a seed agree.
#[test]
fn test_word_save_mid_day_bit_identical() {
    let mut original = World::new(25, Config::load().scaled_to(300));
    assert!(original.config.gossip.enabled);
    original.run_ticks(2 * TICKS_PER_DAY + 700);
    assert!(original.rumours.iter().any(|p| !p.entries.is_empty()));
    assert!(original.reputation.iter().flatten().count() > 0);
    let saved = save::to_ron(&original);
    let mut loaded = save::from_ron(&saved).expect("load");
    original.run_ticks(TICKS_PER_DAY);
    loaded.run_ticks(TICKS_PER_DAY);
    let a = blake3::hash(save::to_ron(&original).as_bytes());
    let b = blake3::hash(save::to_ron(&loaded).as_bytes());
    assert_eq!(a, b, "save/load mid-day diverged with the word on");
    let mut again = World::new(25, Config::load().scaled_to(300));
    again.run_ticks(3 * TICKS_PER_DAY + 700);
    assert_eq!(a, blake3::hash(save::to_ron(&again).as_bytes()), "two runs of a seed agree");
}

/// M15 phase 3 (fix round): a save taken mid-Hunt (a god Hunt under way,
/// its `HuntState` and the derived `hunted_by`) loads and runs on exactly
/// as the world it was taken from.
#[test]
fn test_save_mid_hunt_continues_identically() {
    use citysim::systems::{demography, law};
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY + 600);
    let ads: Vec<EntityId> =
        w.citizens().into_iter().filter(|&a| law::living(&w, a) && demography::is_adult(&w, a)).collect();
    let (hunter, target) = (ads[3], ads[40]);
    w.push_command(PlayerCommand::Hunt { hunter, target });
    w.run_ticks(1);
    assert!(w.hunts.contains_key(&hunter), "the god Hunt is under way");
    // Into the Hunt: until it watches (its target marked hunted), at most a day.
    for _ in 0..TICKS_PER_DAY {
        if w.hunted_by.contains_key(&target) || !w.hunts.contains_key(&hunter) {
            break;
        }
        w.run_ticks(1);
    }
    assert!(!w.hunts.is_empty(), "still hunting at the save");
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(back.hunted_by, w.hunted_by, "hunted_by rebuilt");
    w.run_ticks(TICKS_PER_DAY / 2);
    back.run_ticks(TICKS_PER_DAY / 2);
    assert_eq!(blake3::hash(save::to_ron(&w).as_bytes()), blake3::hash(save::to_ron(&back).as_bytes()));
}

/// M15 phase 3 (fix round): a save taken while a guard stands over a body
/// (`guards_of_corpse` is not saved: it is rebuilt from the guard's plan)
/// loads and runs on exactly as the world it was taken from.
#[test]
fn test_save_mid_guard_continues_identically() {
    use citysim::systems::{demography, grudges, law, lod};
    use citysim::{ActionInstance, ActionKind, Brain, DeathCause, ExecState, GoalKind, Lod, Plan};
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY + 600);
    let ads: Vec<EntityId> =
        w.citizens().into_iter().filter(|&a| law::living(&w, a) && demography::is_adult(&w, a)).collect();
    let (dead, guard) = (ads[5], ads[60]);
    w.edge_entry(guard, dead).kind = RelKind::Family;
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    for x in [dead, guard] {
        lod::set_lod(&mut w, x, Lod::Coarse);
        w.enter_building(x, bar);
    }
    w.kill_by(dead, DeathCause::Violence, None);
    let now = w.tick;
    let step = ActionInstance { action: ActionKind::StakeOut, target: Some(dead), tile: None };
    let b = w.comp_mut::<Brain>(guard).expect("brain");
    b.plan = Some(Plan { goal: GoalKind::GuardBody, target: Some(dead), steps: vec![step], started_tick: now });
    b.plan_step = 0;
    b.current_goal = Some(GoalKind::GuardBody);
    b.exec = ExecState::Use { kind: ActionKind::StakeOut, until: now + 360, started: now };
    grudges::start_guard(&mut w, guard, dead);
    // Saved the tick the wait began (a think may end the wait soon after).
    assert!(grudges::guarding(&w, guard, dead), "still standing over the body");
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(back.guards_of_corpse, w.guards_of_corpse, "the guard is registered again");
    w.run_ticks(600);
    back.run_ticks(600);
    assert_eq!(blake3::hash(save::to_ron(&w).as_bytes()), blake3::hash(save::to_ron(&back).as_bytes()));
}

/// M16a (plan C5, C40): escrow is inside `total_coins`, so L2's identity
/// holds across a save and load with a brokered record open, and the
/// loaded world runs on byte for byte.
#[test]
fn test_save_mid_contract_keeps_identity() {
    use citysim::contract::{ContractKind, Origin, Posting, Target};
    use citysim::systems::{contracts, ownership};
    let mut w = World::new(42, Config::load());
    w.run_ticks(TICKS_PER_DAY / 2);
    let ids: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| citysim::systems::law::living(&w, a) && citysim::systems::demography::is_adult(&w, a))
        .take(2)
        .collect();
    let (buyer, t) = (ids[0], ids[1]);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(buyer) {
        x.coins = 5000;
    }
    let f = contracts::open_fixers(&w)[0];
    let id = contracts::post(
        &mut w,
        Posting {
            buyer: Some(buyer),
            agent: Some(buyer),
            kind: ContractKind::Beat,
            target: Target::Agent(t),
            broker: Some(f),
            deadline_days: 7,
            origin: Origin::God,
            price: None,
        },
    )
    .expect("posted");
    assert!(w.contracts[&id].escrow > 0);
    let ident = |w: &World| ownership::total_coins(w) + w.outside.treasuries() - w.outside.minted;
    let text = save::to_ron(&w);
    let mut back = save::from_ron(&text).expect("load");
    assert_eq!(ident(&back), ident(&w), "the identity across the load");
    assert_eq!(back.escrow_held, w.escrow_held);
    assert_eq!(back.by_party, w.by_party, "the indices rebuilt");
    w.run_ticks(600);
    back.run_ticks(600);
    assert_eq!(blake3::hash(save::to_ron(&w).as_bytes()), blake3::hash(save::to_ron(&back).as_bytes()));
}
