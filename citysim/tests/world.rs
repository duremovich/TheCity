//! M0: map, population, entity arena.

use citysim::{BuildingKind, Config, EntityId, Household, Identity, Job, Position, Role, Wallet, World};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

#[test]
fn test_map_parses_expected_counts() {
    let w = world(1);
    let map = &w.map;
    assert_eq!(map.count_kind(BuildingKind::Home), 60);
    assert_eq!(map.count_kind(BuildingKind::Farm), 2);
    for kind in [
        BuildingKind::Market,
        BuildingKind::Bar,
        BuildingKind::Jail,
        BuildingKind::Cemetery,
        BuildingKind::Hall,
        BuildingKind::Hideout,
        BuildingKind::Warehouse,
    ] {
        assert_eq!(map.count_kind(kind), 1, "{kind}");
    }
    assert_eq!(map.buildings.len(), 69);
    for b in &map.buildings {
        // exactly one Door on the perimeter, and the tile outside it is Road
        let doors = (b.rect.y..b.rect.y + b.rect.h)
            .flat_map(|y| (b.rect.x..b.rect.x + b.rect.w).map(move |x| citysim::TilePos { x, y }))
            .filter(|&p| map.tile_at(p) == citysim::TileKind::Door)
            .count();
        assert_eq!(doors, 1, "{} at {},{}", b.kind, b.rect.x, b.rect.y);
        assert!(map.door_faces_road(b.rect, b.door), "{} door {} does not face a Road", b.kind, b.door);
    }
    // the world spawned one entity per building, in file order
    for (i, b) in map.buildings.iter().enumerate() {
        let id = EntityId { index: i as u32, generation: 0 };
        let building = w.comp::<citysim::Building>(id).expect("building entity");
        assert_eq!(building.kind, b.kind);
        assert_eq!(building.door, b.door);
    }
}

#[test]
fn test_spawn_300_with_jobs() {
    let w = world(42);
    let citizens = w.citizens();
    assert_eq!(citizens.len(), 300);
    assert_eq!(w.population(), 300);

    let count = |role: Role| citizens.iter().filter(|&&id| w.comp::<Job>(id).is_some_and(|j| j.role == role)).count();
    assert_eq!(count(Role::Farmer), 24);
    assert_eq!(count(Role::Guard), 10);
    assert_eq!(count(Role::Clerk), 3);
    assert_eq!(count(Role::Bartender), 2);
    assert_eq!(count(Role::Gravedigger), 1);
    let jobless = citizens.iter().filter(|&&id| !w.has::<Job>(id)).count();
    assert_eq!(jobless, 260);

    for &id in &citizens {
        let home = w.comp::<Household>(id).expect("household").home.expect("everyone has a home");
        let b = w.comp::<citysim::Building>(home).expect("home is a building");
        assert_eq!(b.kind, BuildingKind::Home);
        assert!(b.occupants.contains(&id));
        assert_eq!(w.comp::<Position>(id).expect("position").building, Some(home));
        let wallet = w.comp::<Wallet>(id).expect("wallet");
        assert!((10..=40).contains(&wallet.coins));
        let ident = w.comp::<Identity>(id).expect("identity");
        assert!((18..=60).contains(&ident.age_years()), "age {}", ident.age_years());
        assert!(ident.born_tick < 0);
    }
    // every job has an employer of the right kind
    for &id in &citizens {
        let Some(job) = w.comp::<Job>(id) else { continue };
        let employer = job.employer.expect("employer");
        assert_eq!(w.comp::<citysim::Building>(employer).expect("workplace").kind, job.role.workplace());
    }
    // each home holds exactly 5 residents
    for home in w.buildings_by_kind[&BuildingKind::Home].iter() {
        assert_eq!(w.comp::<citysim::Building>(*home).expect("home").occupants.len(), 5);
    }
    // guards with an even index work nights
    for &id in &citizens {
        let Some(job) = w.comp::<Job>(id) else { continue };
        if job.role == Role::Guard {
            let night = job.shifts == vec![(1260, 1440), (0, 360)];
            assert_eq!(night, id.index % 2 == 0);
        }
    }
}

#[test]
fn test_initial_stocks_and_treasury() {
    let w = world(7);
    let stock = |kind| {
        let id = w.building_of_kind(kind).expect("building");
        w.comp::<citysim::Building>(id).expect("building").stock_food
    };
    assert_eq!(stock(BuildingKind::Market), 600);
    assert_eq!(stock(BuildingKind::Warehouse), 1500);
    assert_eq!(stock(BuildingKind::Farm), 0);
    assert_eq!(stock(BuildingKind::Home), 10);
    assert_eq!(w.treasury().expect("treasury").coins, 5000);
    assert_eq!(w.market().expect("market").price_food, 3);
    let gang = w.gang_id().expect("gang");
    let gang = w.comp::<citysim::Gang>(gang).expect("gang");
    assert_eq!(gang.name, "The Hollow");
    assert_eq!(gang.treasury, 50);
    assert!(gang.members.is_empty());
    assert_eq!(gang.territory, vec![w.building_of_kind(BuildingKind::Hideout).expect("hideout")]);
}

#[test]
fn test_entity_generation_invalidates_stale_id() {
    let mut w = world(1);
    let before = w.generations.len();
    let a = w.spawn();
    w.insert(a, Wallet { coins: 7 });
    assert_eq!(w.comp::<Wallet>(a).map(|x| x.coins), Some(7));

    assert!(w.despawn(a));
    assert!(!w.is_alive(a));
    assert!(w.comp::<Wallet>(a).is_none());
    assert!(!w.despawn(a), "double despawn is a no-op");

    let b = w.spawn();
    assert_eq!(b.index, a.index, "the slot is reused");
    assert_eq!(b.generation, a.generation + 1);
    assert_eq!(w.generations.len(), before + 1, "no new slot was allocated");
    assert!(w.comp::<Wallet>(b).is_none(), "the reused slot starts empty");
    assert!(w.comp::<Wallet>(a).is_none(), "the old id still returns None");
    assert!(!w.is_alive(a));
    assert!(w.is_alive(b));
}
