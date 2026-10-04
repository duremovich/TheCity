//! M6: demography.

use rand::SeedableRng;

use citysim::systems::demography;
use citysim::{
    Brain, Building, BuildingKind, Child, Config, Corpse, DeathCause, EventKind, Household, Identity, Job, Needs,
    Personality, PlayerCommand, Rect, RelKind, Wallet, World, TICKS_PER_DAY,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

fn civilian(w: &World) -> citysim::EntityId {
    w.citizens().into_iter().find(|&id| !w.has::<Job>(id) && w.has::<Brain>(id)).expect("civilian")
}

fn burials(w: &World) -> u32 {
    w.stats.history.iter().map(|r| r.burials).sum::<u32>() + w.stats.current.burials
}

#[test]
fn test_starvation_kills_after_3_days_at_zero() {
    let mut w = world(41);
    let id = civilian(&w);
    let grace = w.config.needs.starvation_grace_ticks;
    assert_eq!(grace, 3 * TICKS_PER_DAY);
    for _ in 0..(grace + 2) {
        if let Some(n) = w.comp_mut::<Needs>(id) {
            n.hunger = 0.0;
        }
        w.tick();
        if w.has::<Corpse>(id) {
            break;
        }
    }
    let corpse = w.comp::<Corpse>(id).expect("a corpse");
    assert_eq!(corpse.cause, DeathCause::Starvation);
    assert!(!w.has::<Brain>(id) && !w.has::<Needs>(id));
    assert!(w.has::<Identity>(id), "Identity stays on the corpse");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Death && e.actors.contains(&id)));
}

#[test]
fn test_old_age_death_probability() {
    let cfg = Config::load().demography;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let age = 95 * citysim::time::DAYS_PER_YEAR as u32;
    let deaths = (0..10_000).filter(|_| demography::old_age_roll(&mut rng, age, &cfg)).count();
    assert!((70..=130).contains(&deaths), "{deaths} deaths");
    assert!(!demography::old_age_roll(&mut rng, 30 * citysim::time::DAYS_PER_YEAR as u32, &cfg));
}

#[test]
fn test_corpse_buried_by_gravedigger_within_2_days() {
    let mut w = world(43);
    let id = civilian(&w);
    let home = w.comp::<Household>(id).and_then(|h| h.home).expect("home");
    assert_eq!(w.comp::<citysim::Position>(id).and_then(|p| p.building), Some(home), "starts at home");
    w.kill(id, DeathCause::Violence);
    assert!(w.has::<Corpse>(id));
    let mut buried_at = None;
    for t in 0..(2 * TICKS_PER_DAY) {
        w.tick();
        if burials(&w) >= 1 {
            buried_at = Some(t);
            break;
        }
    }
    assert!(buried_at.is_some(), "no burial in two days");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Burial && e.actors.contains(&id)));
    if w.is_alive(id) {
        let cemetery = w.building_of_kind(BuildingKind::Cemetery);
        assert_eq!(w.comp::<citysim::Position>(id).and_then(|p| p.building), cemetery);
        assert!(w.comp::<Corpse>(id).is_some_and(|c| c.buried));
    }
}

#[test]
fn test_inheritance_spouse_then_children_then_treasury() {
    // A: the spouse inherits everything.
    let mut w = world(44);
    let (a, b) = {
        let mut it = w.citizens().into_iter().filter(|&id| w.has::<Brain>(id) && w.spouse_of(id).is_none());
        (it.next().expect("a"), it.next().expect("b"))
    };
    w.set_spouse(a, b);
    w.comp_mut::<Wallet>(a).expect("w").coins = 50;
    let before = w.comp::<Wallet>(b).expect("w").coins;
    w.kill(a, DeathCause::OldAge);
    assert_eq!(w.comp::<Wallet>(b).expect("w").coins, before + 50);
    assert!(w.comp::<Identity>(b).expect("i").spouse_died_tick.is_some());

    // B: no spouse; two children split it, remainder to the first.
    let mut w = world(45);
    let (p, q) = {
        let mut it = w.citizens().into_iter().filter(|&id| w.has::<Brain>(id) && w.spouse_of(id).is_none());
        (it.next().expect("p"), it.next().expect("q"))
    };
    let home = w.comp::<Household>(p).and_then(|h| h.home).expect("home");
    let c1 = demography::spawn_child(&mut w, p, q, home);
    let c2 = demography::spawn_child(&mut w, p, q, home);
    w.comp_mut::<Wallet>(p).expect("w").coins = 51;
    w.kill(p, DeathCause::OldAge);
    let (k1, k2) = (w.comp::<Wallet>(c1).expect("w").coins, w.comp::<Wallet>(c2).expect("w").coins);
    assert_eq!(k1 + k2, 51);
    assert_eq!((k1 - k2).abs(), 1);

    // C: nobody: the Treasury.
    let mut w = world(46);
    let lone = w.citizens().into_iter().find(|&id| w.has::<Brain>(id) && w.spouse_of(id).is_none()).expect("lone");
    w.comp_mut::<Wallet>(lone).expect("w").coins = 30;
    let t0 = w.treasury().expect("t").coins;
    w.kill(lone, DeathCause::OldAge);
    assert_eq!(w.treasury().expect("t").coins, t0 + 30);
}

#[test]
fn test_birth_blends_traits_and_links_parents() {
    let mut w = world(47);
    let (m, f) = {
        let mut it = w.citizens().into_iter().filter(|&id| w.has::<Brain>(id));
        (it.next().expect("m"), it.next().expect("f"))
    };
    let home = w.comp::<Household>(m).and_then(|h| h.home).expect("home");
    let child = demography::spawn_child(&mut w, m, f, home);
    assert!(w.has::<Child>(child) && !w.has::<Brain>(child));
    assert_eq!(w.comp::<Identity>(child).expect("i").age_days, 0);
    let (pm, pf, pc) = (
        w.comp::<Personality>(m).expect("p").clone(),
        w.comp::<Personality>(f).expect("p").clone(),
        w.comp::<Personality>(child).expect("p").clone(),
    );
    let axes: [fn(&Personality) -> f32; 6] =
        [|p| p.lawfulness, |p| p.greed, |p| p.pride, |p| p.sociability, |p| p.courage, |p| p.loyalty];
    for ax in axes {
        let mean = (ax(&pm) + ax(&pf)) / 2.0;
        assert!((ax(&pc) - mean).abs() <= 0.45, "trait {} vs mean {mean}", ax(&pc));
    }
    for parent in [m, f] {
        let e = w.edge(parent, child).expect("parent edge");
        assert_eq!(e.kind, RelKind::Parent);
        assert!((e.affinity - 0.6).abs() < 1e-6 && (e.trust - 0.8).abs() < 1e-6);
    }
    assert!(w.events.iter().any(|e| e.kind == EventKind::Birth && e.actors.contains(&child)));
    assert_eq!(w.stats.current.births, 1);
}

#[test]
fn test_immigration_two_per_week() {
    let mut w = world(48);
    let start = w.population();
    w.run_ticks(28 * TICKS_PER_DAY + 1);
    let immigrants: u32 = w.stats.history.iter().map(|r| r.immigrants).sum::<u32>() + w.stats.current.immigrants;
    assert_eq!(immigrants, 8);
    let arrivals = w.events.iter().filter(|e| e.kind == EventKind::Immigration).count();
    assert!(arrivals >= 2, "the last week's arrivals are still in the log");
    for id in w.citizens() {
        assert!(w.has::<Household>(id), "every citizen has a Household (housed or homeless)");
    }
    let _ = start;
}

#[test]
fn test_demolish_home_makes_residents_homeless() {
    let mut w = world(49);
    let home = w.buildings_by_kind[&BuildingKind::Home][3];
    let residents: Vec<_> =
        w.citizens().into_iter().filter(|&c| w.comp::<Household>(c).and_then(|h| h.home) == Some(home)).collect();
    assert!(!residents.is_empty());
    w.push_command(PlayerCommand::DemolishHome(home));
    w.tick();
    for &r in &residents {
        assert_eq!(w.comp::<Household>(r).and_then(|h| h.home), None);
        assert_ne!(w.comp::<citysim::Position>(r).and_then(|p| p.building), Some(home));
    }
    let homeless_events = w.events.iter().filter(|e| e.kind == EventKind::Homeless).count();
    assert_eq!(homeless_events, residents.len());
    assert!(w.comp::<Building>(home).is_some_and(|b| b.demolished));
    assert!(!w.buildings_by_kind[&BuildingKind::Home].contains(&home));
}

#[test]
fn test_build_home_rejects_invalid_rect() {
    let mut w = world(50);
    let failed = |w: &World| w.events.iter().filter(|e| e.kind == EventKind::PlayerActionFailed).count();
    // on Water (the bottom rows)
    w.push_command(PlayerCommand::BuildHome { rect: Rect { x: 10, y: 60, w: 4, h: 4 } });
    w.tick();
    assert_eq!(failed(&w), 1);
    // overlapping an existing building
    let b = w.comp::<Building>(w.buildings_by_kind[&BuildingKind::Home][0]).expect("b").rect;
    w.push_command(PlayerCommand::BuildHome { rect: Rect { x: b.x, y: b.y, w: 4, h: 4 } });
    w.tick();
    assert_eq!(failed(&w), 2);
    // a valid rect, but the Treasury is short
    let homes_before = w.buildings_by_kind[&BuildingKind::Home].len();
    let ground = find_ground_rect(&w).expect("a free 4x4 of Ground");
    w.treasury_mut().expect("t").coins = 10;
    w.push_command(PlayerCommand::BuildHome { rect: ground });
    w.tick();
    assert_eq!(failed(&w), 3);
    // and with money it goes up
    w.treasury_mut().expect("t").coins = 500;
    w.push_command(PlayerCommand::BuildHome { rect: ground });
    w.tick();
    assert_eq!(failed(&w), 3);
    assert_eq!(w.buildings_by_kind[&BuildingKind::Home].len(), homes_before + 1);
    assert_eq!(w.treasury().expect("t").coins, 300);
}

/// A 4×4 all-Ground rect not touching Water or any building, scanning the map.
fn find_ground_rect(w: &World) -> Option<Rect> {
    for y in 1..(citysim::MAP_H as u8 - 6) {
        for x in 1..(citysim::MAP_W as u8 - 6) {
            let r = Rect { x, y, w: 4, h: 4 };
            let mut ok = true;
            for yy in (y as i32 - 1)..=(y as i32 + 4) {
                for xx in (x as i32 - 1)..=(x as i32 + 4) {
                    let t = citysim::TilePos { x: xx as u8, y: yy as u8 };
                    let kind = w.map.tile_at(t);
                    let inside = r.contains(t);
                    if (inside && kind != citysim::TileKind::Ground) || kind == citysim::TileKind::Water {
                        ok = false;
                    }
                    if w.with::<Building>()
                        .into_iter()
                        .any(|b| w.comp::<Building>(b).is_some_and(|bd| bd.rect.contains(t)))
                    {
                        ok = false;
                    }
                }
            }
            if ok {
                return Some(r);
            }
        }
    }
    None
}
