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
