//! Save/load round trip.

use citysim::{save, BuildingKind, Config, PlayerCommand, World};

#[test]
fn test_save_load_bit_identical() {
    let mut original = World::new(42, Config::load());
    original.run_ticks(3000);
    let saved = save::to_ron(&original);

    let mut loaded = save::from_ron(&saved).expect("load");
    assert_eq!(loaded.tick, 3000);

    original.run_ticks(1000);
    loaded.run_ticks(1000);
    let a = blake3::hash(save::to_ron(&original).as_bytes());
    let b = blake3::hash(save::to_ron(&loaded).as_bytes());
    assert_eq!(a, b);
}

#[test]
fn test_save_round_trips_commands_and_events() {
    let mut w = World::new(5, Config::load());
    w.push_command(PlayerCommand::ReleaseReserve { amount: 100 });
    w.push_command(PlayerCommand::SetTaxRate(0.2));
    w.run_ticks(10);
    assert_eq!(w.command_log.len(), 2);
    assert_eq!(w.events.len(), 2);
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    assert_eq!(w.comp::<citysim::Building>(market).expect("market").stock_food, 700);

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
    let mut w = World::new(9, Config::load());
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
