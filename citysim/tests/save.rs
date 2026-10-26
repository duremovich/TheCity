//! Save/load round trip.

use citysim::{save, BuildingKind, Config, PlayerCommand, World};

#[test]
fn test_save_round_trips_commands_and_events() {
    let mut w = World::new(5, Config::load().dole_city().v1_profile());
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
