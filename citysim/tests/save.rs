//! Save/load round trip.

use citysim::{save, BuildingKind, Config, PlayerCommand, World, TICKS_PER_DAY};

#[test]
fn test_save_load_bit_identical() {
    let mut original = World::new(42, Config::load().v1_profile());
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

/// Drop one `field:(...)` (and its trailing comma) from compact RON, found by
/// `token`, the field's opening text (e.g. `gangs:(` or `hideout:(index:`).
fn strip_field(text: &str, token: &str) -> String {
    let start = text.find(token).unwrap_or_else(|| panic!("{token} not in save"));
    let open = start + token.find('(').expect("token holds the opening paren");
    let mut depth = 0usize;
    let mut end = open;
    for (i, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let end = if text[end..].starts_with(',') { end + 1 } else { end };
    format!("{}{}", &text[..start], &text[end..])
}

/// A save from before M8 has no `[gangs]` config and no `Gang.hideout`:
/// loading one reads the gang config from the assets and hands the gangs
/// their Hideouts in map order (`World::migrate_legacy`).
#[test]
fn test_legacy_save_without_gangs_config_loads() {
    let mut w = World::new(11, Config::load().v1_profile());
    w.run_ticks(100);
    let gangs = w.gangs();
    let home = w.buildings_by_kind[&BuildingKind::Home][0];
    w.comp_mut::<citysim::Gang>(gangs[0]).expect("gang").territory = vec![home];
    let mut text = strip_field(&save::to_ron(&w), "gangs:(names");
    for _ in 0..gangs.len() {
        text = strip_field(&text, "hideout:(index:");
    }
    assert!(!text.contains("hideout:(index:"), "every Gang.hideout stripped");
    let back = save::from_ron(&text).expect("a pre-M8 save loads");
    assert_eq!(back.config.gangs.names, w.config.gangs.names);
    let expected: Vec<_> = gangs.iter().map(|&g| w.hideout_of(g)).collect();
    let got: Vec<_> = gangs.iter().map(|&g| back.hideout_of(g)).collect();
    assert_eq!(got, expected, "Hideouts handed out in map order");
    assert_eq!(
        back.comp::<citysim::Building>(home).expect("home").claim,
        Some(citysim::Claim { gang: gangs[0], count: 3 }),
        "held territory gets a full claim"
    );
}

/// Drop one `field:[...]` (and its trailing comma) from compact RON.
fn strip_list_field(text: &str, token: &str) -> String {
    let start = text.find(token).unwrap_or_else(|| panic!("{token} not in save"));
    let open = start + token.find('[').expect("token holds the opening bracket");
    let mut depth = 0usize;
    let mut end = open;
    for (i, ch) in text[open..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let end = if text[end..].starts_with(',') { end + 1 } else { end };
    format!("{}{}", &text[..start], &text[end..])
}

/// A save from before M9 has no `[law]` config and no `law` store: loading
/// one reads the config from the assets and puts a default `Law` on the Jail.
#[test]
fn test_legacy_save_without_law_loads() {
    let mut w = World::new(13, Config::load().v1_profile());
    w.run_ticks(100);
    let text = save::to_ron(&w);
    let text = strip_field(&text, "law:(window_days");
    let text = strip_list_field(&text, "law:[");
    assert!(!text.contains("law:["), "the law store is stripped");
    let back = save::from_ron(&text).expect("a pre-M9 save loads");
    assert_eq!(back.config.law.window_days, w.config.law.window_days);
    let law = back.law().expect("a default Law on the Jail");
    assert_eq!(law.posture, citysim::Posture::Patrol);
    assert_eq!(back.law.len(), back.alive.len(), "the store covers every entity");
    let mut back = back;
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.law().is_some_and(|l| l.captain.is_some()), "the captain is found at the first midnight");
}

/// M9 renamed `LocationKey::RivalHideout` to `RaidTarget`; a save taken
/// mid-raid still loads through the serde alias.
#[test]
fn test_rival_hideout_location_key_alias_loads() {
    let key: citysim::LocationKey = ron::from_str("RivalHideout").expect("alias");
    assert_eq!(key, citysim::LocationKey::RaidTarget);
    let step: citysim::ActionKind = ron::from_str("GoTo(RivalHideout)").expect("alias in a step");
    assert_eq!(step, citysim::ActionKind::GoTo(citysim::LocationKey::RaidTarget));
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

/// M11 (plan D40, D49): a save from before ownership has no `[rent]` or
/// `[corps]` config and no `corp` store. It loads with M9 behaviour (rent 0,
/// no corps, no payday rent), runs, and every gang reads `Dictator`; a fresh
/// save round-trips a corp's `Governance`.
#[test]
fn test_pre_m11_save_loads() {
    let mut w = World::new(17, Config::load().v1_profile());
    w.run_ticks(100);
    let text = save::to_ron(&w);
    let text = strip_field(&text, "rent:(base");
    let text = strip_field(&text, "corps:(names");
    let text = strip_list_field(&text, "corp:[");
    assert!(!text.contains("corp:[") && !text.contains("rent:(base"), "the M11 keys are stripped");
    let mut back = save::from_ron(&text).expect("a pre-M11 save loads");
    assert!(back.corps().is_empty());
    assert_eq!(back.config.rent.base, [0, 0, 0]);
    assert!(!back.config.rent.pay_from_income);
    assert_eq!(back.config.rent.sump_spoilage_mult, 1.0, "M9 spoilage");
    assert!(back.config.corps.names.is_empty());
    assert_eq!(back.corp.len(), back.alive.len(), "the store covers every entity");
    for g in back.gangs() {
        assert_eq!(back.comp::<citysim::Gang>(g).expect("gang").governance, citysim::Governance::Dictator);
    }
    back.run_ticks(TICKS_PER_DAY);
    assert_eq!(back.stats.history.back().map(|r| r.evictions), Some(0));
    // A current save with corps keeps them, and their governance, bit for bit.
    let mut m11 = World::new(17, Config::load());
    m11.run_ticks(10);
    let corp = m11.corps()[0];
    m11.comp_mut::<citysim::Corp>(corp).expect("corp").governance =
        citysim::Governance::Board { members: vec![citysim::EntityId::NONE] };
    let text = save::to_ron(&m11);
    let again = save::from_ron(&text).expect("loads");
    assert_eq!(save::to_ron(&again), text);
    assert_eq!(
        again.comp::<citysim::Corp>(corp).expect("corp").governance,
        m11.comp::<citysim::Corp>(corp).expect("c").governance
    );
}
