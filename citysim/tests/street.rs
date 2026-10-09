//! M12 phase 3: litter and sanitation, Hotels, derelicts, squats and the
//! Dreg share (docs/M12_DISTRICTS.md § 3-4, plan D16-D28).

use citysim::config::{ControlWeights, DistrictsCfg, LitterCfg, StreetCfg};
use citysim::systems::{classes, corps, districts, faction, gang, law, litter, lod, ownership, plan, street};
use citysim::{
    trace_flags, ActionInstance, ActionKind, Brain, Building, BuildingKind, Class, Config, Corp, Crime, DistrictId,
    EntityId, EventKind, ExecState, GoalKind, Household, Job, LocationKey, Lod, Needs, Niche, Order, Personality, Plan,
    Position, Sentence, Squatter, Stance, TilePos, Wallet, World, TICKS_PER_DAY,
};

/// The v1 city cut into three Mid districts by x, the district law on, and
/// litter and the street on with nothing seeded (the tests seed by hand).
fn street_cfg() -> Config {
    let assets = Config::load();
    let mut c = Config::load().v1_profile();
    c.districts = DistrictsCfg {
        names: vec!["West".into(), "Centre".into(), "East".into()],
        zones: vec!["M".into(), "M".into(), "M".into()],
        x_from: vec![0, 32, 64],
        x_to: vec![32, 64, 256],
        control_min_share: 0.4,
        control_weights: ControlWeights { held_home: 1.0, owned_home: 1.0, owned_other: 3.0, city_per_coverage: 1.0 },
    };
    c.law.district_beats = true;
    c.litter = assets.litter.clone();
    c.street = assets.street.clone();
    c.street.seed_derelict_blocks = 0;
    c.street.seed_hotels = 0;
    c.street.relet_days = 0;
    c
}

fn city() -> World {
    World::new(7, street_cfg())
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

fn idx(w: &World, p: TilePos) -> usize {
    usize::from(p.y) * w.map.w() + usize::from(p.x)
}

/// A street tile whose whole 3 × 3 neighbourhood is street, in district `d`.
fn open_street(w: &World, d: u8) -> TilePos {
    for y in 2..w.map.h() - 2 {
        for x in 2..w.map.w() - 2 {
            let p = TilePos { x: x as u8, y: y as u8 };
            if w.district_of(p) != DistrictId(d) {
                continue;
            }
            let all = (-1..=1i32).all(|dy| {
                (-1..=1i32).all(|dx| w.is_street(TilePos { x: (x as i32 + dx) as u8, y: (y as i32 + dy) as u8 }))
            });
            if all {
                return p;
            }
        }
    }
    panic!("no open street in district {d}");
}

/// Adults with a Brain, not guards or gang members, free, ascending.
fn civilians(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && citysim::systems::demography::is_adult(w, a))
        .filter(|&a| !law::is_guard(w, a) && w.gang_of(a).is_none() && !w.has::<Sentence>(a))
        .collect()
}

/// Put an agent out on the street at `tile`, homeless.
fn sleep_rough(w: &mut World, a: EntityId, tile: TilePos) {
    w.leave_building(a);
    w.set_home(a, None);
    let p = w.comp_mut::<Position>(a).expect("position");
    p.tile = tile;
    p.building = None;
}

/// The city's first Bar turned into a Capsule Hotel owned by `owner`.
fn make_hotel(w: &mut World, owner: Option<EntityId>) -> EntityId {
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    for s in ownership::staff_at(w, bar) {
        citysim::systems::economy::dismiss(w, s, Some(bar), "test".into());
    }
    w.vacancies.remove(&bar);
    {
        let b = w.comp_mut::<Building>(bar).expect("bar");
        b.kind = BuildingKind::Hotel;
        b.capacity = 12;
    }
    w.buildings_by_kind.get_mut(&BuildingKind::Bar).expect("bars").retain(|&b| b != bar);
    w.buildings_by_kind.entry(BuildingKind::Hotel).or_default().push(bar);
    ownership::transfer_building(w, bar, owner);
    districts::rebuild(w);
    bar
}

/// The `n`th Home of district `d`, made derelict.
fn derelict_in(w: &mut World, d: u8, n: usize) -> EntityId {
    let h = w.district(DistrictId(d)).homes[n];
    assert!(street::make_derelict(w, h, "test"));
    h
}

// ---------------------------------------------------------------------------
// Litter (D16-D19)
// ---------------------------------------------------------------------------

#[test]
fn test_litter_deposits_per_event_and_decays() {
    let mut w = city();
    // The spec's table at scale 1 (the fix pass's `deposit_mult` has its own test).
    w.config.litter.deposit_mult = 1.0;
    let t = open_street(&w, 0);
    let actor = civilians(&w)[0];
    // A Theft: 6 at the tile, nothing around it.
    law::raise_crime(&mut w, actor, None, Crime::Theft, t);
    assert_eq!(litter::at(&w, t), 6);
    let east = TilePos { x: t.x + 1, y: t.y };
    assert_eq!(litter::at(&w, east), 0, "r 0: the tile alone");
    // An Assault elsewhere: 12 at the centre, 6 on the ring (half at the rim).
    let u = open_street(&w, 2);
    law::raise_crime(&mut w, actor, None, Crime::Assault, u);
    assert_eq!(litter::at(&w, u), 12);
    for (dx, dy) in [(-1i32, -1i32), (0, -1), (1, 1), (-1, 0)] {
        let p = TilePos { x: (i32::from(u.x) + dx) as u8, y: (i32::from(u.y) + dy) as u8 };
        assert_eq!(litter::at(&w, p), 6, "ring tile {p}");
    }
    // A Murder deposits nothing here (its death does, 40 r1 in `kill_by`).
    let before: u32 = w.litter.iter().map(|&v| u32::from(v)).sum();
    let m = open_street(&w, 1);
    law::raise_crime(&mut w, actor, None, Crime::Murder, m);
    assert_eq!(w.litter.iter().map(|&v| u32::from(v)).sum::<u32>(), before);
    // One midnight: -1 each.
    litter::decay(&mut w);
    assert_eq!(litter::at(&w, t), 5);
    assert_eq!(litter::at(&w, u), 11);
    assert_eq!(litter::at(&w, east), 0);
    // Fix pass: a district's litter is the share of its street tiles at or
    // above `visible` (32): a 5 is no visible mark.
    litter::district_means(&mut w);
    let d0 = w.district(DistrictId(0));
    assert_eq!(d0.litter, 0.0);
    assert_eq!(d0.streets.len() as u32, d0.walk_tiles);
}

/// Fix pass (items 14, 18, 19): a deposit is scaled by `deposit_mult`, an
/// event inside a building lands at its outside door, and the district's
/// litter is the share of street tiles at or above `visible`.
#[test]
fn test_litter_scaled_anchored_at_the_door_and_read_as_a_share() {
    let mut w = city();
    w.config.litter.deposit_mult = 8.0;
    w.config.litter.visible = 32;
    let actor = civilians(&w)[0];
    // A Theft on the street: 6 x 8 = 48, a visible mark.
    let t = open_street(&w, 0);
    law::raise_crime(&mut w, actor, None, Crime::Theft, t);
    assert_eq!(litter::at(&w, t), 48);
    // A Theft inside a Home (its interior tile) lands outside its door.
    let home = w.district(DistrictId(1)).homes[0];
    let (rect, outside) = {
        let b = w.comp::<Building>(home).expect("home");
        (b.rect, w.outside_door(b))
    };
    let inside = TilePos { x: rect.x + 1, y: rect.y + 1 };
    assert!(!w.is_street(inside) && w.is_street(outside));
    let before = litter::at(&w, outside);
    litter::deposit_near(&mut w, inside, Some(home), 6, 0);
    assert_eq!(litter::at(&w, outside), before + 48, "the street outside the door takes it");
    // Capped at 254: a violent death (40 x 8) is heaped, not rubble.
    let u = open_street(&w, 2);
    litter::deposit(&mut w, u, 40, 0);
    assert_eq!(litter::at(&w, u), 254);
    // The share: tiles >= 32 over the district's street tiles.
    litter::district_means(&mut w);
    let d0 = w.district(DistrictId(0));
    let dirty = d0.streets.iter().filter(|&&i| w.litter[i as usize] >= 32).count();
    assert!(dirty >= 1);
    assert!((d0.litter - dirty as f32 / d0.streets.len() as f32).abs() < 1e-6);
}

#[test]
fn test_deposit_skips_building_tiles_and_saturates_at_254() {
    let mut w = city();
    let home = w.district(DistrictId(0)).homes[0];
    let door = w.comp::<Building>(home).expect("home").door;
    litter::deposit(&mut w, door, 50, 0);
    assert_eq!(litter::at(&w, door), 0, "a door is inside the building's rect: no street");
    let t = open_street(&w, 0);
    litter::deposit(&mut w, t, 200, 0);
    litter::deposit(&mut w, t, 200, 0);
    assert_eq!(litter::at(&w, t), 254, "litter never becomes rubble");
    // Off: no deposits at all.
    w.config.litter.enabled = false;
    let u = open_street(&w, 2);
    litter::deposit(&mut w, u, 50, 1);
    assert_eq!(litter::at(&w, u), 0);
}

#[test]
fn test_littered_step_delays_next_move_and_keeps_flow_fields() {
    let mut w = city();
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    let a = civilians(&w)[0];
    lod::set_lod(&mut w, a, Lod::Full);
    // On the street ten tiles from the Market's door.
    let door = w.comp::<Building>(market).expect("market").door;
    let start = (0..w.map.h())
        .flat_map(|y| (0..w.map.w()).map(move |x| TilePos { x: x as u8, y: y as u8 }))
        .filter(|&p| w.is_street(p))
        .min_by_key(|&p| (p.manhattan(door).abs_diff(10), p))
        .expect("a street tile");
    sleep_rough(&mut w, a, start);
    let first = w.flow_step(market, start).expect("a step toward the Market");
    let fields = w.flow_fields.len();
    let i = idx(&w, first);
    w.litter[i] = 200; // heaped: band 3
    assert_eq!(citysim::exec::litter_delay(&w, first), 2);
    assert_eq!(citysim::exec::litter_delay(&w, start), 0, "a clean tile delays nothing");
    let now = w.tick;
    {
        let b = w.comp_mut::<Brain>(a).expect("brain");
        b.plan = Some(Plan {
            goal: GoalKind::Idle,
            target: Some(market),
            steps: vec![ActionInstance {
                action: ActionKind::GoTo(LocationKey::Market),
                target: Some(market),
                tile: None,
            }],
            started_tick: now,
        });
        b.plan_step = 0;
        b.exec = ExecState::Idle;
    }
    citysim::exec::run(&mut w); // the walk starts
    citysim::exec::run(&mut w); // the first step, onto the heaped tile
    assert_eq!(w.comp::<Position>(a).expect("pos").tile, first);
    let move_ticks = w.config.exec.move_ticks_full;
    match &w.comp::<Brain>(a).expect("brain").exec {
        ExecState::Goto { next_move_tick, .. } => assert_eq!(*next_move_tick, now + move_ticks + 2),
        other => panic!("walking, got {other:?}"),
    }
    assert_eq!(w.flow_fields.len(), fields, "litter never touches the flow cache");
    assert!(w.flow_fields.contains(market));
}

#[test]
fn test_sanitation_removes_units_dirtiest_first() {
    let mut w = city();
    let streets = w.district(DistrictId(1)).streets.clone();
    let (a, b, c) = (streets[10] as usize, streets[20] as usize, streets[30] as usize);
    w.litter[a] = 200;
    w.litter[b] = 100;
    w.litter[c] = 50;
    let used = litter::clean_dirtiest(&mut w, DistrictId(1), 250);
    assert_eq!(used, 250);
    assert_eq!((w.litter[a], w.litter[b], w.litter[c]), (0, 50, 50));
}

// ---------------------------------------------------------------------------
// Hotels (D20-D22)
// ---------------------------------------------------------------------------

#[test]
fn test_hotel_books_charges_owner_price_and_blocks_vagrancy() {
    let mut w = city();
    let inn = ownership::spawn_corp(&mut w, "Inn".into(), [Niche::Food].into_iter().collect(), 0, None);
    w.comp_mut::<Corp>(inn).expect("corp").price_level.insert(Niche::Food, 1.5);
    let hotel = make_hotel(&mut w, Some(inn));
    assert_eq!(street::hotel_price(&w, hotel), 5, "(3 × 1.5).round() = 5");
    let guest = civilians(&w)[0];
    lod::set_lod(&mut w, guest, Lod::Statistical);
    let at = w.district(DistrictId(0)).centroid;
    sleep_rough(&mut w, guest, at);
    w.comp_mut::<Wallet>(guest).expect("wallet").coins = 10;
    let (purse, city) = (w.purse(Some(inn)), w.purse(None));
    street::book_statistical(&mut w);
    assert_eq!(street::booked_hotel(&w, guest), Some(hotel));
    assert_eq!(w.comp::<Wallet>(guest).expect("wallet").coins, 5);
    let (to_owner, tax) = (w.purse(Some(inn)) - purse, w.purse(None) - city);
    assert_eq!(to_owner + tax, 5, "the night's price, the tax withheld for the Treasury");
    assert!(to_owner >= 4);
    assert_eq!(w.stats.current.hotel_nights, 1);
    assert_eq!(street::free_beds(&w, hotel), 11);
    // No Vagrancy roll for a guest, even at p = 1.
    w.config.law.vagrancy_base = 1.0;
    for d in &mut w.districts {
        d.coverage = 1.0;
    }
    assert!(!street::rough_sleepers(&w).contains(&guest));
    street::vagrancy(&mut w);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::Vagrancy && e.actors.contains(&guest)));
    // A night in a booked bed is a bed (SLEPT_AT_HOME) and a Hotel night.
    assert_eq!(classes::class_of(&w, guest), Class::Dreg, "a guest stays a Dreg");
    // The booking ends at 08:00.
    let until = w.hotel_beds[&guest].1;
    assert_eq!(until % TICKS_PER_DAY, 8 * 60);
    w.tick = until;
    street::expire_bookings(&mut w);
    assert_eq!(street::booked_hotel(&w, guest), None);
}

#[test]
fn test_full_homeless_plans_hotel_when_affordable() {
    let mut w = city();
    let hotel = make_hotel(&mut w, None);
    let outside = {
        let b = w.comp::<Building>(hotel).expect("hotel");
        w.outside_door(b)
    };
    let a = civilians(&w)[0];
    lod::set_lod(&mut w, a, Lod::Full);
    sleep_rough(&mut w, a, outside);
    w.comp_mut::<Needs>(a).expect("needs").energy = 0.1;
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 10;
    assert!(street::hotel_available(&w, a));
    w.comp_mut::<Brain>(a).expect("brain").current_goal = Some(GoalKind::Sleep);
    plan::plan_for(&mut w, a, GoalKind::Sleep);
    let steps: Vec<ActionKind> =
        w.comp::<Brain>(a).and_then(|b| b.plan.clone()).expect("a plan").steps.iter().map(|s| s.action).collect();
    assert_eq!(steps, vec![ActionKind::GoTo(LocationKey::Hotel), ActionKind::CheckIn, ActionKind::Sleep]);
    // Broke: the street.
    w.comp_mut::<Brain>(a).expect("brain").plan = None;
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    assert!(!street::hotel_available(&w, a));
    plan::plan_for(&mut w, a, GoalKind::Sleep);
    let steps: Vec<ActionKind> =
        w.comp::<Brain>(a).and_then(|b| b.plan.clone()).expect("a plan").steps.iter().map(|s| s.action).collect();
    assert_eq!(steps.last(), Some(&ActionKind::Sleep));
    assert!(!steps.contains(&ActionKind::CheckIn));
    // Checked in, the night is paid and the Sleep is a bed's.
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 10;
    w.enter_building(a, hotel);
    assert!(street::check_in(&mut w, a, hotel).is_ok());
    assert_eq!(w.comp::<Wallet>(a).expect("wallet").coins, 7);
    let safety = w.comp::<Needs>(a).expect("needs").safety;
    let now = w.tick;
    citysim::exec::actions::on_complete(&mut w, a, ActionKind::Sleep, None, now, now);
    assert!(w.comp::<Needs>(a).expect("needs").safety > safety, "a bed's safety, not the street's");
    let marks = w.day_marks.get(&a).copied().unwrap_or(0);
    assert!(marks & trace_flags::SLEPT_AT_HOME != 0 && marks & trace_flags::HOTEL != 0);
}

// ---------------------------------------------------------------------------
// Derelicts (D25, D26)
// ---------------------------------------------------------------------------

#[test]
fn test_bankruptcy_below_floor_leaves_derelict() {
    let mut w = city();
    let co =
        ownership::spawn_corp(&mut w, "Doomed".into(), [Niche::Housing, Niche::Food].into_iter().collect(), -1, None);
    let block = w.district(DistrictId(0)).homes[0];
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    ownership::transfer_building(&mut w, block, Some(co));
    ownership::transfer_building(&mut w, market, Some(co));
    let residents = w.residents_of(block).to_vec();
    assert!(!residents.is_empty());
    let floor = w.config.street.city_absorb_floor;
    w.treasury_mut().expect("treasury").coins = floor - 1;
    corps::bankrupt(&mut w, co);
    let b = w.comp::<Building>(block).expect("block");
    assert!(b.derelict && b.owner.is_none() && b.rent_per_day == 0, "an unsold Block goes derelict");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Derelict && e.actors.contains(&block)));
    for r in residents {
        assert_eq!(w.comp::<Household>(r).and_then(|h| h.home), None, "its residents are on the street");
    }
    assert!(!w.district(DistrictId(0)).homes.contains(&block), "a derelict is no Home");
    let m = w.comp::<Building>(market).expect("market");
    assert!(!m.derelict && m.owner.is_none(), "a Market goes to the City");
    // Above the floor the City takes a Block, as in M11.
    let co2 = ownership::spawn_corp(&mut w, "Doomed2".into(), [Niche::Housing].into_iter().collect(), -1, None);
    let other = w.district(DistrictId(0)).homes[1];
    ownership::transfer_building(&mut w, other, Some(co2));
    w.treasury_mut().expect("treasury").coins = floor * 2;
    corps::bankrupt(&mut w, co2);
    let o = w.comp::<Building>(other).expect("block");
    assert!(!o.derelict && o.owner.is_none());
}

#[test]
fn test_seed_derelicts_spread_over_sump() {
    let mut cfg = Config::load();
    cfg.street.seed_derelict_blocks = 8;
    // M15 W31: The Unplugged's Chapel would take one of the eight.
    cfg.creeds.seed_purist = false;
    let mut w = World::new(42, cfg.clone());
    let derelicts = street::derelicts(&w);
    assert_eq!(derelicts.len(), 8);
    for d in [5u8, 6, 7] {
        let n = derelicts.iter().filter(|&&b| w.district_of_building(b) == DistrictId(d)).count();
        assert!(n >= 2, "{} has {n}", w.district_name(DistrictId(d)));
    }
    for &b in &derelicts {
        let bd = w.comp::<Building>(b).expect("b");
        assert!(bd.owner.is_none() && bd.kind == BuildingKind::Home && bd.stock_food == 0);
        assert!(w.residents_of(b).is_empty());
    }
    let homeless =
        w.citizens().into_iter().filter(|&a| w.comp::<Household>(a).is_some_and(|h| h.home.is_none())).count();
    assert_eq!(homeless, 40);
    // The opening Hotels stand in two Sump districts.
    let hotels = w.buildings_of_kind(BuildingKind::Hotel).to_vec();
    assert_eq!(hotels.len(), cfg.street.seed_hotels);
    assert!(count(&w, EventKind::Founded) >= hotels.len());
    // The world stream is the one a world without derelicts draws.
    cfg.street.seed_derelict_blocks = 0;
    let mut v = World::new(42, cfg);
    use rand::Rng;
    assert_eq!(w.rng.world().random::<u64>(), v.rng.world().random::<u64>());
}

#[test]
fn test_restore_evicts_squatters() {
    let mut w = city();
    let b = derelict_in(&mut w, 0, 0);
    let civ = civilians(&w);
    for &a in &civ[..2] {
        let at = w.district(DistrictId(0)).centroid;
        sleep_rough(&mut w, a, at);
        street::occupy(&mut w, a, b).expect("a slot");
    }
    assert_eq!(w.squatters_of(b).len(), 2);
    assert!(street::restore(&mut w, b, None, "bought"));
    assert!(w.squatters_of(b).is_empty());
    assert_eq!(count(&w, EventKind::SquatEvicted), 2);
    let bd = w.comp::<Building>(b).expect("b");
    assert!(!bd.derelict && bd.capacity == w.config.buildings.home.capacity);
    assert!(w.district(DistrictId(0)).homes.contains(&b), "a Home again");
    for &a in &civ[..2] {
        assert!(!w.has::<Squatter>(a));
        assert!(street::squat_banned(&w, a, b));
    }
    // Nationalising a derelict is a restore, at no price.
    let c = derelict_in(&mut w, 0, 1);
    let treasury = w.purse(None);
    assert!(ownership::nationalise(&mut w, c).is_ok());
    assert!(!w.comp::<Building>(c).expect("c").derelict);
    assert_eq!(w.purse(None), treasury);
}

// ---------------------------------------------------------------------------
// Squats (D27, D38)
// ---------------------------------------------------------------------------

#[test]
fn test_occupy_makes_squatter_who_stays_dreg() {
    let mut w = city();
    let b = derelict_in(&mut w, 1, 0);
    let a = civilians(&w)[0];
    lod::set_lod(&mut w, a, Lod::Full);
    let at = w.district(DistrictId(1)).centroid;
    sleep_rough(&mut w, a, at);
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    // Housed agents cannot squat; the homeless can, with a slot in reach.
    assert!(street::can_squat(&w, a) || street::squat_target(&w, a).is_none());
    street::occupy(&mut w, a, b).expect("a slot");
    assert_eq!(count(&w, EventKind::Squatted), 1);
    assert_eq!(w.comp::<Squatter>(a).map(|s| s.building), Some(b));
    assert_eq!(w.squatters_of(b), &[a]);
    assert_eq!(classes::class_of(&w, a), Class::Dreg, "a squatter stays a Dreg");
    assert!(!street::rough_sleepers(&w).contains(&a), "no Vagrancy roll in a squat");
    // A second squatter does not log Squatted again.
    let c = civilians(&w)[1];
    sleep_rough(&mut w, c, at);
    street::occupy(&mut w, c, b).expect("a slot");
    assert_eq!(count(&w, EventKind::Squatted), 1);
    // Sleep in the squat: +0.1 safety and the SQUAT mark.
    w.enter_building(a, b);
    w.comp_mut::<Needs>(a).expect("needs").safety = 0.5;
    let now = w.tick;
    citysim::exec::actions::on_complete(&mut w, a, ActionKind::Sleep, None, now, now);
    assert!((w.comp::<Needs>(a).expect("needs").safety - 0.6).abs() < 1e-6);
    assert!(w.day_marks.get(&a).copied().unwrap_or(0) & trace_flags::SQUAT != 0);
    // Housed: the squat ends (`set_home` ends it).
    let home = w.district(DistrictId(1)).homes[0];
    w.set_home(a, Some(home));
    assert!(!w.has::<Squatter>(a));
    // A long sentence ends one too.
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let until = w.tick + 5 * TICKS_PER_DAY;
    law::sentence(&mut w, c, Crime::Theft, until, jail);
    assert!(!w.has::<Squatter>(c));
    assert!(w.squatters_of(b).is_empty());
    assert!(w.check_indices().is_ok());
}

#[test]
fn test_sweep_clears_one_squat() {
    let mut w = city();
    let (big, small) = (derelict_in(&mut w, 0, 0), derelict_in(&mut w, 0, 1));
    let civ = civilians(&w);
    let at = w.district(DistrictId(0)).centroid;
    for (i, &a) in civ[..3].iter().enumerate() {
        lod::set_lod(&mut w, a, Lod::Statistical);
        sleep_rough(&mut w, a, at);
        street::occupy(&mut w, a, if i < 2 { big } else { small }).expect("slot");
    }
    w.districts[0].stance = Stance::Sweep;
    let caught = street::sweep_squats(&mut w);
    assert_eq!(caught.len(), 2, "one squat a night: the fuller one");
    assert!(w.squatters_of(big).is_empty());
    assert_eq!(w.squatters_of(small), &[civ[2]]);
    assert_eq!(count(&w, EventKind::SquatEvicted), 2);
    for &a in &civ[..2] {
        assert!(street::squat_banned(&w, a, big));
        // Rolled as a Vagrancy arrest at p = 1: fined or jailed.
        assert!(
            w.events.iter().any(|e| e.kind == EventKind::Vagrancy && e.actors.contains(&a)) || w.has::<Sentence>(a)
        );
    }
}

#[test]
fn test_gang_squat_holds_derelict() {
    let mut w = city();
    let g = w.gang_list()[0];
    let hideout_d = w.district_of_building(w.hideout_of(g).expect("hideout")).0;
    let b = derelict_in(&mut w, hideout_d, 0);
    assert_eq!(gang::squat_targets(&w, g), vec![b]);
    let inputs = faction::gather_inputs(&w, g);
    if let Some(i) = inputs {
        assert_eq!(i.derelicts, 1);
        let scores = faction::score_orders(&i, &w.config.gangs);
        assert!(scores.iter().any(|s| s.order == Order::Squat), "the Squat order is scored");
    }
    // A meek squatter is in when the gang moves in.
    let civ = civilians(&w);
    let meek = civ[0];
    let at = w.district(DistrictId(hideout_d)).centroid;
    sleep_rough(&mut w, meek, at);
    street::occupy(&mut w, meek, b).expect("slot");
    w.comp_mut::<Personality>(meek).expect("p").courage = 0.1;
    let member = civ[1];
    gang::enlist(&mut w, member, g);
    w.comp_mut::<Personality>(member).expect("p").loyalty = 1.0;
    w.comp_mut::<citysim::Gang>(g).expect("gang").order = Order::Squat;
    w.enter_building(member, b);
    assert!(gang::serving_squat(&w, member));
    for _ in 0..gang::CLAIM_HELD {
        gang::squat_claim(&mut w, member, b);
    }
    let claim = w.comp::<Building>(b).and_then(|x| x.claim).expect("claimed");
    assert_eq!((claim.gang, claim.count), (g, gang::CLAIM_HELD));
    assert!(w.comp::<citysim::Gang>(g).expect("gang").territory.contains(&b), "held territory");
    assert!(!w.has::<Squatter>(meek), "the meek are put out");
    assert!(w.events.iter().any(|e| e.kind == EventKind::SquatEvicted && e.actors.contains(&meek)));
    // The held squat counts for the gang's presence, not the City's.
    let d = DistrictId(hideout_d);
    let pres = districts::presence(&w, d);
    assert!(pres.iter().any(|&(c, _)| c == citysim::Controller::Gang(g)));
    assert!(gang::squat_targets(&w, g).is_empty(), "nothing left to take");
}

// ---------------------------------------------------------------------------
// The Dreg share and the rubble hook
// ---------------------------------------------------------------------------

#[test]
fn test_dreg_share_on_hand_built_world() {
    let mut w = city();
    let hotel = make_hotel(&mut w, None);
    let b = derelict_in(&mut w, 1, 0);
    let civ: Vec<EntityId> = civilians(&w).into_iter().filter(|&a| !w.has::<Job>(a)).collect();
    let at = w.district(DistrictId(1)).centroid;
    for &a in &civ[..6] {
        lod::set_lod(&mut w, a, Lod::Statistical);
        sleep_rough(&mut w, a, at);
        w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    }
    // Two squat, two take a bed, two sleep rough.
    street::occupy(&mut w, civ[0], b).expect("slot");
    street::occupy(&mut w, civ[1], b).expect("slot");
    for &a in &civ[2..4] {
        w.comp_mut::<Wallet>(a).expect("wallet").coins = 3;
    }
    street::book_statistical(&mut w);
    assert_eq!(street::booked_hotel(&w, civ[2]), Some(hotel));
    assert_eq!(street::booked_hotel(&w, civ[3]), Some(hotel));
    let homeless: usize = w
        .citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && citysim::systems::demography::is_adult(&w, a))
        .filter(|&a| w.comp::<Household>(a).is_some_and(|h| h.home.is_none()))
        .count();
    // Every one of them is a Dreg (v1 has no corps, so no homeless Corp worker).
    classes::run(&mut w);
    let counts: Vec<u32> = w.classes.iter().map(|c| c.count).collect();
    assert_eq!(counts[2] as usize, homeless);
    assert!(homeless >= 6);
    let adults: u32 = counts.iter().sum();
    let share = f64::from(counts[2]) / f64::from(adults);
    assert!((share - homeless as f64 / f64::from(adults)).abs() < 1e-9);
    // The CSV's columns: class_dreg, squatters, derelicts, hotel nights.
    w.run_ticks(TICKS_PER_DAY);
    let row = w.stats.history.back().expect("a row");
    assert!(row.class_dreg >= 2, "the squatters at least are still Dregs ({})", row.class_dreg);
    assert!(row.derelicts >= 1);
    assert!(row.hotel_nights >= 2);
}

#[test]
fn test_rubble_hook_blocks_and_invalidates() {
    let mut w = city();
    let t = open_street(&w, 0);
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    w.flow_step(market, t);
    // Off by default: a no-op.
    litter::set_rubble(&mut w, t, true);
    assert_eq!(litter::at(&w, t), 0);
    assert!(w.map.walkable(t));
    assert!(!w.flow_fields.is_empty());
    // On: rubble is a wall, the cached fields go; cleared, it is street again.
    w.config.litter.rubble_blocks = true;
    litter::set_rubble(&mut w, t, true);
    assert_eq!(litter::at(&w, t), litter::RUBBLE);
    assert!(!w.map.walkable(t));
    assert!(w.flow_fields.is_empty());
    litter::deposit(&mut w, t, 10, 0);
    assert_eq!(litter::at(&w, t), litter::RUBBLE, "a deposit leaves rubble be");
    litter::set_rubble(&mut w, t, false);
    assert_eq!(litter::at(&w, t), 0);
    assert!(w.map.walkable(t));
}

// ---------------------------------------------------------------------------
// Saves
// ---------------------------------------------------------------------------

fn strip(text: &str, token: &str, open: char, close: char) -> String {
    let start = text.find(token).unwrap_or_else(|| panic!("{token} not in save"));
    let from = start + token.find(open).expect("token holds the opener");
    let mut depth = 0usize;
    let mut end = from;
    for (i, ch) in text[from..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                end = from + i + 1;
                break;
            }
        }
    }
    let end = if text[end..].starts_with(',') { end + 1 } else { end };
    format!("{}{}", &text[..start], &text[end..])
}

/// D46: a save from before the street (no `[litter]`/`[street]` config, no
/// litter grid, no bookings, sweep beats or squatter store) loads with zero
/// litter and the street off, and runs a day as M11 did.
#[test]
fn test_pre_m12_street_save_loads_with_zero_litter() {
    let mut cfg = Config::load().scaled_to(300);
    cfg.litter = LitterCfg::off();
    cfg.street = StreetCfg::off();
    let mut w = World::new(19, cfg);
    w.run_ticks(TICKS_PER_DAY + 10);
    let text = citysim::save::to_ron(&w);
    let text = strip(&text, "litter:(enabled", '(', ')');
    let text = strip(&text, "street:(enabled", '(', ')');
    let text = strip(&text, "litter:[", '[', ']');
    let text = strip(&text, "squatter:[", '[', ']');
    let text = strip(&text, "hotel_beds:{", '{', '}');
    let text = strip(&text, "sweep_beats:{", '{', '}');
    let text = text.replace(",derelict:false,full_capacity:None,empty_since:None", "");
    let text = text.replace(",raided_at:None", "");
    assert!(!text.contains("litter:[") && !text.contains("squatter:[") && !text.contains("derelict:"));
    let mut back = citysim::save::from_ron(&text).expect("a pre-street save loads");
    assert_eq!(back.config.litter, LitterCfg::off());
    assert_eq!(back.config.street, StreetCfg::off());
    assert_eq!(back.litter.len(), back.map.w() * back.map.h());
    assert!(back.litter.iter().all(|&v| v == 0));
    assert_eq!(back.squatter.len(), back.alive.len());
    back.run_ticks(TICKS_PER_DAY);
    assert!(back.litter.iter().all(|&v| v == 0), "no litter with it off");
    assert!(back.buildings_of_kind(BuildingKind::Hotel).is_empty());
    assert!(street::derelicts(&back).is_empty());
    assert!(back.check_indices().is_ok());
    // A current save keeps its street (litter, squats, bookings) bit for bit.
    let mut cur = city();
    let b = derelict_in(&mut cur, 0, 0);
    let a = civilians(&cur)[0];
    let at = cur.district(DistrictId(0)).centroid;
    sleep_rough(&mut cur, a, at);
    street::occupy(&mut cur, a, b).expect("slot");
    let t = open_street(&cur, 0);
    litter::deposit(&mut cur, t, 90, 2);
    let text = citysim::save::to_ron(&cur);
    let again = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&again), text);
    assert_eq!(again.squatters_of(b), &[a]);
    assert_eq!(again.litter, cur.litter);
}

// ---------------------------------------------------------------------------
// M12 fix pass (phase 2 and 3 reviews)
// ---------------------------------------------------------------------------

/// Item 20: any path to a Home (here `set_home`, as a marriage takes) ends
/// a squat and tonight's Hotel booking.
#[test]
fn test_set_home_ends_squat_and_booking() {
    let mut w = city();
    let b = derelict_in(&mut w, 0, 0);
    let home = w.district(DistrictId(0)).homes[1];
    let a = civilians(&w)[0];
    let at = w.district(DistrictId(0)).centroid;
    sleep_rough(&mut w, a, at);
    street::occupy(&mut w, a, b).expect("slot");
    w.hotel_beds.insert(a, (b, w.tick + 100));
    w.set_home(a, Some(home));
    assert!(!w.has::<Squatter>(a), "the squat ends");
    assert!(!w.hotel_beds.contains_key(&a), "the booking ends");
    assert!(w.squatters_of(b).is_empty());
    assert!(w.check_indices().is_ok());
}

/// Items 21, 24, 25: the City re-lets a derelict of any kind (a Bar here) at
/// the capacity it had, but not a squat a gang holds.
#[test]
fn test_relet_any_kind_keeps_capacity_and_skips_gang_held() {
    let mut w = city();
    w.config.street.relet_days = 1;
    w.treasury_mut().expect("treasury").coins = w.config.street.city_absorb_floor * 2;
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    w.comp_mut::<Building>(bar).expect("bar").capacity = 7;
    let home = w.district(DistrictId(0)).homes[0];
    assert!(street::make_derelict(&mut w, bar, "test"));
    assert!(street::make_derelict(&mut w, home, "test"));
    // The Block is a gang's held squat.
    let g = w.gang_list()[0];
    w.comp_mut::<Building>(home).expect("b").claim = Some(citysim::Claim { gang: g, count: gang::CLAIM_HELD });
    w.tick += 2 * TICKS_PER_DAY;
    street::relet_daily(&mut w);
    street::relet_daily(&mut w);
    let b = w.comp::<Building>(bar).expect("bar");
    assert!(!b.derelict, "a derelict Bar returns to use");
    assert_eq!(b.capacity, 7, "at the capacity it stood at");
    assert!(street::is_derelict(&w, home), "a gang-held squat is not re-let");
}

/// Item 21: a Bar with no trade for `abandon_days` under an owner in the red
/// is abandoned like an empty Block.
#[test]
fn test_idle_bar_of_a_broke_owner_is_abandoned() {
    let mut w = city();
    let co = ownership::spawn_corp(&mut w, "Broke".into(), [Niche::Food].into_iter().collect(), -50, None);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    ownership::transfer_building(&mut w, bar, Some(co));
    w.comp_mut::<Corp>(co).expect("corp").treasury = -50;
    w.comp_mut::<Building>(bar).expect("bar").revenue = std::iter::once(0).collect();
    let days = w.config.street.abandon_days;
    for _ in 0..=days {
        street::abandon_daily(&mut w);
        w.tick += TICKS_PER_DAY;
    }
    assert!(street::is_derelict(&w, bar), "an idle Bar in the red goes derelict");
}

/// Item 27: a bankruptcy marks every door of the estate, sold or foreclosed.
#[test]
fn test_bankruptcy_litters_every_door() {
    let mut w = city();
    w.config.litter.deposit_mult = 1.0;
    let co = ownership::spawn_corp(&mut w, "Doomed".into(), [Niche::Food].into_iter().collect(), -1, None);
    let market = w.buildings_of_kind(BuildingKind::Market)[0];
    ownership::transfer_building(&mut w, market, Some(co));
    let outside = {
        let b = w.comp::<Building>(market).expect("m");
        w.outside_door(b)
    };
    let before = litter::at(&w, outside);
    corps::bankrupt(&mut w, co);
    assert_eq!(litter::at(&w, outside), before + 48, "48 r2 at the door (anchored outside)");
}

/// Items 10 and 11: only the sleepers the law can roll count as rough, and a
/// district without Homes rolls at coverage 1, not coverage_max.
#[test]
fn test_vagrancy_counts_rollable_sleepers_and_homeless_district_coverage() {
    let mut w = city();
    let civ = civilians(&w);
    let at = open_street(&w, 0);
    let (out, inside) = (civ[0], civ[1]);
    lod::set_lod(&mut w, out, Lod::Full);
    lod::set_lod(&mut w, inside, Lod::Full);
    sleep_rough(&mut w, out, at);
    sleep_rough(&mut w, inside, at);
    let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
    w.enter_building(inside, bar);
    let d = w.district_of(at);
    w.config.law.vagrancy_base = 0.0;
    street::vagrancy(&mut w);
    let inside_d = w.district_of(w.comp::<Position>(inside).expect("p").tile);
    let rough: u16 = w.districts.iter().map(|x| x.rough).sum();
    assert_eq!(rough, 1, "the Full sleeper inside the Bar is not rough ({:?} / {:?})", d, inside_d);
    // A district without Homes reads coverage_max but rolls at 1.
    w.config.law.vagrancy_base = 0.1;
    let i = d.index();
    w.districts[i].coverage = 2.0;
    let homes = std::mem::take(&mut w.districts[i].homes);
    assert!((street::vagrancy_p(&w, d) - 0.1).abs() < 1e-6, "{}", street::vagrancy_p(&w, d));
    w.districts[i].homes = homes;
    assert!((street::vagrancy_p(&w, d) - 0.2).abs() < 1e-6);
}
