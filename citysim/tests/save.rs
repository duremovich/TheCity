//! Save/load round trip.

use std::collections::BTreeMap;

use citysim::{save, BuildingKind, Config, Edge, EntityId, PlayerCommand, RelKind, World, TICKS_PER_DAY};

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

/// M12 D46: a save from before districts has no `[districts]` config, no
/// `districts`, `district_watch`, `report_places` or `eviction_places`:
/// loading one rebuilds the grid and the rows with empty aggregates, and it
/// runs.
#[test]
fn test_pre_m12_save_loads() {
    let mut w = World::new(19, Config::load().scaled_to(300));
    w.run_ticks(TICKS_PER_DAY + 10);
    let text = save::to_ron(&w);
    let text = strip_field(&text, "districts:(names");
    // The world's districts and every DayRow's district slots.
    let mut text = text;
    while text.contains("districts:[") {
        text = strip_list_field(&text, "districts:[");
    }
    let text = strip_field(&text, "district_watch:(today");
    let text = strip_list_field(&text, "report_places:[");
    let text = strip_list_field(&text, "eviction_places:[");
    assert!(!text.contains("districts:") && !text.contains("district_watch:"), "the M12 keys are stripped");
    let mut back = save::from_ron(&text).expect("a pre-M12 save loads");
    assert_eq!(back.config.districts, w.config.districts, "the assets' cuts");
    assert_eq!(back.districts.len(), 8);
    assert_eq!(back.district_grid, w.district_grid, "the grid is rebuilt");
    for (a, b) in back.districts.iter().zip(&w.districts) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.homes, b.homes);
        assert_eq!(a.population, 0, "aggregates start empty");
        assert!(!a.control_init);
    }
    // The zone watch moved to each zone's first district.
    let moved: u32 = back.district_watch.yesterday.iter().sum();
    assert_eq!(moved, back.zone_watch.yesterday.iter().sum::<u32>());
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.districts.iter().any(|d| d.population > 0), "the next midnight aggregates");
    assert!(back.districts.iter().all(|d| d.control_init));
    // A current save keeps its districts bit for bit.
    let text = save::to_ron(&w);
    assert_eq!(save::to_ron(&save::from_ron(&text).expect("loads")), text);
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

/// M12: a save from before the packed format (edges as a map of key ->
/// field-named struct) still loads, to the same edges.
#[test]
fn test_save_loads_legacy_edge_map() {
    let mut w = World::new(7, Config::load().v1_profile());
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(!w.edges.is_empty());
    let text = save::to_ron(&w);
    // The old serializer: the ordered map of `Edge` structs.
    let legacy_map: BTreeMap<(EntityId, EntityId), Edge> = w.edges.iter().map(|(&k, e)| (k, e.clone())).collect();
    let legacy_edges = ron::to_string(&legacy_map).expect("legacy edges");
    assert!(legacy_edges.starts_with('{') && legacy_edges.contains("affinity:"));
    let start = text.find("edges:\"e1:").expect("packed edges in save") + "edges:".len();
    let end = start + 1 + text[start + 1..].find('"').expect("closing quote") + 1;
    let legacy = format!("{}{}{}", &text[..start], legacy_edges, &text[end..]);
    assert!(legacy.len() > text.len());

    let back = save::from_ron(&legacy).expect("legacy load");
    assert_eq!(edge_bits(&back), edge_bits(&w));
    assert_eq!(save::to_ron(&back), text);
}

/// Drop one `field:{...}` (and its trailing comma) from compact RON.
fn strip_map_field(text: &str, token: &str) -> String {
    let start = text.find(token).unwrap_or_else(|| panic!("{token} not in save"));
    let open = start + token.find('{').expect("token holds the opening brace");
    let mut depth = 0usize;
    let mut end = open;
    for (i, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
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

/// M13 D50: a save from before assets has no `asset`, `body`, `appearance`
/// or `trips` and no `[assets]`/`[chrome]` config: it loads with assets
/// off, every agent gets a Body from its keyed stream, every Kit is bare,
/// and a day runs.
#[test]
fn test_pre_m13_save_loads() {
    let mut w = World::new(23, Config::load().scaled_to(300));
    w.run_ticks(TICKS_PER_DAY + 10);
    let text = save::to_ron(&w);
    let text = strip_list_field(&text, "asset:[");
    let text = strip_list_field(&text, "body:[");
    let text = strip_list_field(&text, "appearance:[");
    let text = strip_map_field(&text, "trips:{");
    let text = strip_field(&text, "assets:(enabled");
    let text = strip_field(&text, "chrome:(arms_fighting");
    assert!(
        !text.contains("asset:[") && !text.contains("body:[") && !text.contains("trips:"),
        "the M13 keys are stripped"
    );
    let mut back = save::from_ron(&text).expect("a pre-M13 save loads");
    assert!(!back.config.assets.enabled, "assets off");
    assert_eq!(back.asset.len(), back.alive.len(), "the stores cover every entity");
    assert_eq!(back.body.len(), back.alive.len());
    for id in back.citizens() {
        let body = back.comp::<citysim::Body>(id).expect("a backfilled Body");
        assert!((0.2..0.6).contains(&body.strength) && body.sanity == 1.0);
        assert!(back.comp::<citysim::Kit>(id).expect("a Kit").is_bare());
    }
    back.check_indices().expect("indices in step");
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.population() > 0);
    // A current save keeps its Bodies bit for bit.
    let text = save::to_ron(&w);
    assert_eq!(save::to_ron(&save::from_ron(&text).expect("loads")), text);
}

/// Drop every `,<key>:<number>` (a scalar struct field) from compact RON.
fn strip_scalar_fields(text: &str, key: &str) -> String {
    let token = format!(",{key}:");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(&token) {
        out.push_str(&rest[..i]);
        let tail = &rest[i + token.len()..];
        let end = tail.find([',', ')']).unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// M14 V44: a save from before the plane has no `virt`, `db` or runs, no
/// `Corp.tech` and no `Skills.hacking`: it loads with the plane built and
/// its ICE seeded, the trees seeded by name, every hacking backfilled from
/// its keyed stream, and a day runs.
#[test]
fn test_pre_m14_save_loads() {
    let mut w = World::new(23, Config::load().scaled_to(300));
    w.run_ticks(TICKS_PER_DAY + 10);
    let text = save::to_ron(&w);
    let mut text = strip_field(&text, "virt:(nodes:[");
    for token in ["db:{", "runs:{", "run_orders:{", "last_trace:{"] {
        text = strip_map_field(&text, token);
    }
    for token in ["run_queue:[", "run_log:["] {
        text = strip_list_field(&text, token);
    }
    while text.contains("tech:(tier") {
        text = strip_field(&text, "tech:(tier");
    }
    let text = strip_scalar_fields(&text, "hacking");
    assert!(
        !text.contains("virt:(nodes:[") && !text.contains("tech:(tier") && !text.contains("hacking:"),
        "the M14 keys are stripped"
    );
    let mut back = save::from_ron(&text).expect("a pre-M14 save loads");
    assert!(back.config.virt.enabled);
    assert!(back.virt.alive_count() >= 20, "the plane is built: {} nodes", back.virt.alive_count());
    assert!(back.virt.nodes.iter().all(|n| n.store.total() == 0), "no stores: the Research order builds the Labs");
    let z = back
        .corps()
        .into_iter()
        .find(|&c| back.comp::<citysim::Corp>(c).is_some_and(|cc| cc.name == "Zetatech"))
        .expect("Zetatech");
    assert_eq!(back.comp::<citysim::Corp>(z).expect("corp").tech.tier, [3, 1, 3], "seeded by name");
    for id in back.citizens() {
        let h = back.comp::<citysim::Skills>(id).expect("skills").hacking;
        assert!((0.0..=1.0).contains(&h), "hacking backfilled: {h}");
    }
    back.check_indices().expect("indices in step");
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.population() > 0);
}
