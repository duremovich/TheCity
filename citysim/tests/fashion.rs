//! Jobs & lives phase 4, fashion (docs/JOBS_AND_LIVES.md § 9 and § 12's
//! `fashion.rs` unit tests, with Dylan's answer 1: dress from the worn
//! clothing item). `dress = max(derived, the worn item's)`, fading with its
//! condition; the Lounge's door and a row's `min_dress` read it; a Boutique
//! visit raises `status` and sells an item; the save keeps it all.

use citysim::systems::{assets, fashion, leisure};
use citysim::{Asset, AssetKind, BuildingKind, Class, Config, EntityId, Needs, World};

fn on_world() -> World {
    World::new(42, Config::load())
}

fn dress(w: &World, a: EntityId) -> u8 {
    citysim::systems::reputation::appearance_of(w, a).dress
}

/// A free Street-class adult (lowest id).
fn street_adult(w: &World) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            citysim::systems::demography::is_adult(w, a)
                && citysim::systems::classes::class_of(w, a) == Class::Street
                && !w.has::<citysim::Sentence>(a)
                && dress(w, a) <= 1
        })
        .expect("a Street adult")
}

#[test]
fn test_dress_is_the_worn_item_when_better_and_fades() {
    let mut w = on_world();
    assets::run(&mut w);
    let a = street_adult(&w);
    assert_eq!(dress(&w, a), 1, "derived: Street");
    let item = fashion::grant(&mut w, a, 3).expect("a T3 item");
    assert_eq!(fashion::worn(&w, a), Some(item));
    assets::run(&mut w);
    assert_eq!(dress(&w, a), 3, "the worn T3 item");
    w.comp_mut::<Asset>(item).expect("item").condition = 40;
    assets::run(&mut w);
    assert_eq!(dress(&w, a), 1, "T3 at 40 % reads 1.2: the derived 1 holds");
    // Off, the item is no reader's.
    w.config.fashion.enabled = false;
    assert_eq!(fashion::worn_dress(&w, a), 0);
}

#[test]
fn test_lounge_door_and_min_dress_read_it() {
    let mut w = on_world();
    assets::run(&mut w);
    let a = street_adult(&w);
    let lounge = w.buildings_of_kind(BuildingKind::Lounge)[0];
    assert!(!leisure::door_ok(&w, a, lounge), "a dress of 1 is refused");
    let companion = w.config.trade_role("companion").expect("the companion row");
    assert!(!fashion::dress_ok(&w, a, companion), "a companion is hired at dress 2");
    fashion::grant(&mut w, a, 2).expect("a T2 item");
    assets::run(&mut w);
    assert!(leisure::door_ok(&w, a, lounge), "the worn item lets him in");
    assert!(fashion::dress_ok(&w, a, companion));
}

#[test]
fn test_boutique_visit_raises_status_and_dresses() {
    let mut w = on_world();
    assets::run(&mut w);
    let a = street_adult(&w);
    let b = w.buildings_of_kind(BuildingKind::Boutique)[0];
    w.comp_mut::<Needs>(a).expect("needs").status = 0.1;
    w.comp_mut::<citysim::Wallet>(a).expect("wallet").coins = 1000;
    fashion::on_visit(&mut w, a, b, false);
    assert!(w.comp::<Needs>(a).is_some_and(|n| n.status > 0.4), "status raised");
    let worn = fashion::worn(&w, a).expect("an item bought and worn");
    assert_eq!(w.comp::<Asset>(worn).map(|x| x.kind), Some(AssetKind::Clothing));
    let j2 = w.stats.current.jobs2.clone().unwrap_or_default();
    assert_eq!((j2.boutique_visits, j2.clothes_sold), (1, 1));
    // The day's decay.
    fashion::daily(&mut w);
    assert!(w.comp::<Needs>(a).is_some_and(|n| n.status < 0.45 + 1e-6));
}

#[test]
fn test_fashion_off_spawns_nothing_and_save_round_trips() {
    let mut c = Config::load();
    c.fashion.enabled = false;
    let off = World::new(42, c);
    assert!(off.buildings_of_kind(BuildingKind::Boutique).is_empty());
    assert!(off.jobs2.fashion.is_default());
    let mut w = on_world();
    assets::run(&mut w);
    let a = street_adult(&w);
    fashion::grant(&mut w, a, 2).expect("an item");
    w.comp_mut::<Needs>(a).expect("needs").status = 0.3;
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(citysim::save::to_ron(&back), text, "byte-identical round trip");
    assert_eq!(back.jobs2.fashion, w.jobs2.fashion);
    assert!(back.comp::<Needs>(a).is_some_and(|n| (n.status - 0.3).abs() < 1e-6));
    assert_eq!(fashion::worn(&back, a), fashion::worn(&w, a));
}
