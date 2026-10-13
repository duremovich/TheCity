//! M17 phase 5 (the rename hazard): "Arasaka", "Militech", "Nutrix" and
//! "Zetatech" are placeholder names. Logic keys on a row (`[corps]
//! fab_rows`, `noodle_first_row`, `seed_labs`, `seed_feeds`) or on config
//! fields, never on a name; the references the config makes by name resolve
//! to a row (`Config::unresolved_names`).

use citysim::components::{Building, BuildingKind, Corp};
use citysim::{Config, World};

#[test]
fn test_shipped_config_names_resolve() {
    let cfg = Config::load();
    assert_eq!(cfg.unresolved_names(), Vec::<String>::new());
    assert_eq!(cfg.clone().scaled_to(20_000).unresolved_names(), Vec::<String>::new());
}

#[test]
fn test_a_renamed_row_without_its_references_is_caught() {
    let mut cfg = Config::load();
    let i = cfg.corps.names.iter().position(|n| n == "Zetatech").expect("the Tech row");
    cfg.corps.names[i] = "Kiroshi".into();
    let bad = cfg.unresolved_names();
    assert!(bad.iter().any(|l| l.contains("[tech] seed") && l.contains("Zetatech")), "{bad:?}");
    cfg.corps.fab_rows.push(99);
    assert!(cfg.unresolved_names().iter().any(|l| l.contains("fab_rows names row 99")));
}

/// A renamed row keeps its seeded Labs and Fabs: the seeding reads the row.
#[test]
fn test_seeding_follows_the_row_not_the_name() {
    let mut cfg = Config::load();
    let i = cfg.corps.names.iter().position(|n| n == "Zetatech").expect("the Tech row");
    cfg.corps.names[i] = "Kiroshi".into();
    if let Some(t) = cfg.tech.seed.remove("Zetatech") {
        cfg.tech.seed.insert("Kiroshi".into(), t);
    }
    assert_eq!(cfg.unresolved_names(), Vec::<String>::new());
    let w = World::new(42, cfg);
    let kiroshi =
        w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == "Kiroshi")).expect("Kiroshi");
    let owned = |kind: BuildingKind| {
        w.buildings_of_kind(kind)
            .iter()
            .filter(|&&b| w.comp::<Building>(b).and_then(|x| x.owner) == Some(kiroshi))
            .count()
    };
    assert_eq!(owned(BuildingKind::Lab), 2, "the row's two seeded Labs");
    assert!(owned(BuildingKind::Fab) >= 1, "a seeded Fab");
}

/// Review: the Food row renamed keeps the Noodle Bars' first place and its
/// Feed ("<owner> Now"): `noodle_first_row` and `seed_feeds` read the row.
#[test]
fn test_food_row_renamed_keeps_its_noodle_bars_and_feed() {
    let mut cfg = Config::load();
    let i = cfg.corps.names.iter().position(|n| n == "Nutrix").expect("the Food row");
    cfg.corps.names[i] = "Synthco".into();
    for b in cfg.governance.boards.iter_mut().filter(|b| b.as_str() == "Nutrix") {
        *b = "Synthco".into();
    }
    assert_eq!(cfg.unresolved_names(), Vec::<String>::new());
    let w = World::new(42, cfg);
    let synthco =
        w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == "Synthco")).expect("Synthco");
    // The renamed row owns the Noodle Bars and the Feed the shipped row owns.
    let shipped = World::new(42, Config::load());
    let nutrix = shipped
        .corps()
        .into_iter()
        .find(|&c| shipped.comp::<Corp>(c).is_some_and(|x| x.name == "Nutrix"))
        .expect("Nutrix");
    let bars = |w: &World, c| {
        w.buildings_of_kind(BuildingKind::NoodleBar)
            .iter()
            .filter(|&&b| w.comp::<Building>(b).and_then(|x| x.owner) == Some(c))
            .count()
    };
    assert!(bars(&shipped, nutrix) > 0);
    assert_eq!(bars(&w, synthco), bars(&shipped, nutrix), "the row's Noodle Bars");
    let feeds: Vec<String> = w
        .buildings_of_kind(BuildingKind::Feed)
        .iter()
        .filter_map(|&b| w.comp::<Building>(b).and_then(|x| x.feed.as_ref()).map(|f| f.name.clone()))
        .collect();
    assert!(feeds.iter().any(|n| n == "Synthco Now"), "{feeds:?}");
    assert!(!feeds.iter().any(|n| n.contains("Nutrix")), "{feeds:?}");
}
