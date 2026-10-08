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

/// M15 (plan W44): a save from before the word has no heard stores, pools,
/// reputation or regard: it loads with the stores sized, and one day with
/// the word on fills the pools.
#[test]
fn test_pre_m15_save_loads() {
    let mut w = World::new(24, Config::load().scaled_to(300));
    // L1: three days (was one): with the walk to a far Bar weighed, the
    // first exchanges come after day 1.
    w.run_ticks(3 * TICKS_PER_DAY + 10);
    assert!(w.citizens().iter().any(|&id| w.comp::<citysim::Memory>(id).is_some_and(|m| !m.heard.is_empty())));
    let mut text = save::to_ron(&w);
    if text.contains("anon_heard:[") {
        text = strip_list_field(&text, "anon_heard:[");
    }
    while text.contains("heard:[") {
        text = strip_list_field(&text, "heard:[");
    }
    for token in ["rumours:[", "reputation:[", "kill_watch:[", "kill_known:[", "arrest_log:["] {
        text = strip_list_field(&text, token);
    }
    let text = strip_map_field(&text, "regard:{");
    assert!(
        !text.contains("heard:[")
            && !text.contains("rumours:[")
            && !text.contains("reputation:[")
            && !text.contains("regard:{"),
        "the M15 keys are stripped"
    );
    let mut back = save::from_ron(&text).expect("a pre-M15 save loads");
    assert_eq!(back.reputation.len(), back.alive.len(), "the reputation store is sized");
    assert_eq!(back.rumours.len(), back.districts.len(), "one pool per district");
    assert!(back.rumours.iter().all(|p| p.entries.is_empty()));
    back.check_indices().expect("indices in step");
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.rumours.iter().any(|p| !p.entries.is_empty()), "the pools fill");
    assert!(back.reputation.iter().flatten().count() > 0, "reputation built at the first midnight");
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

/// M15 (plan W25, W28, W44): a save from before the social skills (all four
/// 0, no skill means) loads with each agent's skills re-drawn from its own
/// Skill stream and the city means computed.
#[test]
fn test_pre_m15_save_backfills_skills() {
    let mut w = World::new(26, Config::load().scaled_to(300));
    let ids: Vec<citysim::EntityId> = w.with::<citysim::Skills>();
    let seeded: Vec<[f32; 4]> =
        ids.iter().map(|&id| w.comp::<citysim::Skills>(id).map(|s| s.social_all()).unwrap_or_default()).collect();
    assert!(seeded.iter().any(|s| s.iter().any(|&v| v > 0.0)), "seeded at World::new");
    for &id in &ids {
        if let Some(s) = w.comp_mut::<citysim::Skills>(id) {
            s.set_social_all([0.0; 4]);
        }
    }
    w.skill_means = [0.0; 8];
    let back = save::from_ron(&save::to_ron(&w)).expect("loads");
    assert!(back.skill_means.iter().all(|&m| m > 0.0), "means computed: {:?}", back.skill_means);
    for (k, &id) in ids.iter().enumerate() {
        let got = back.comp::<citysim::Skills>(id).map(|s| s.social_all()).unwrap_or_default();
        assert_eq!(got, seeded[k], "backfilled from the same keyed draw");
    }
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

/// L2 (plan L33): a version-1 save (no L2 keys) loads as the M15 city; with
/// its config's L2 turned on, the venues and Fabs are seeded at the first
/// midnight (when Lots exist) and the save reads version 2.
#[test]
fn test_pre_l2_save_loads() {
    let mut w = World::new(26, Config::load().living_off().scaled_to(300));
    w.run_ticks(TICKS_PER_DAY / 2);
    let text = save::to_ron(&w);
    assert!(text.contains(",save_version:2"), "the current format is 2");
    let v1 = text.replace(",save_version:2", ",save_version:1");
    let back = save::from_ron(&v1).expect("a version-1 save loads");
    assert_eq!(back.save_version, 2, "migrated to the current format");
    assert!(!back.config.living.enabled && !back.venues_due, "L2 stays off");
    // The same save with L2 switched on in its config.
    let on = v1
        .replace("living:(enabled:false", "living:(enabled:true")
        .replace("jobs:(enabled:false", "jobs:(enabled:true");
    assert_ne!(on, v1, "the L2 switches are in the saved config");
    let mut back = save::from_ron(&on).expect("loads with L2 on");
    assert!(back.venues_due && !back.venues_seeded, "seeding deferred to the first midnight");
    assert!(back.buildings_of_kind(BuildingKind::NoodleBar).is_empty());
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.venues_seeded && !back.venues_due);
    assert!(!back.buildings_of_kind(BuildingKind::NoodleBar).is_empty(), "the venues stand");
    back.check_indices().expect("indices in step");
}
