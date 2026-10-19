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
    let assets = Config::load().dole_city();
    let mut c = Config::load().dole_city().v1_profile();
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
            let p = TilePos { x: x as u16, y: y as u16 };
            if w.district_of(p) != DistrictId(d) {
                continue;
            }
            let all = (-1..=1i32).all(|dy| {
                (-1..=1i32).all(|dx| w.is_street(TilePos { x: (x as i32 + dx) as u16, y: (y as i32 + dy) as u16 }))
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
        let p = TilePos { x: (i32::from(u.x) + dx) as u16, y: (i32::from(u.y) + dy) as u16 };
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
        .flat_map(|y| (0..w.map.w()).map(move |x| TilePos { x: x as u16, y: y as u16 }))
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
    let mut cfg = Config::load().dole_city();
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
    // Jobs & lives phase 6: a cult's chapel takes the hand-built derelict at
    // the first midnight.
    w.config.faith.enabled = false;
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
    let mut cfg = Config::load().dole_city().scaled_to(300);
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

mod common;

/// Lives at work phase 4 (docs/LIVES_AT_WORK.md § 2.3, § 9.1): the street
/// and freight. Unit tests in the shipped seed-42 city with the
/// preconditions set by hand; each failed on the base `1f37f78` (no
/// `[service] street`). The probes are in `tests/person.rs`.
mod lives_street {

    use super::common::{self, coins, set_coins};
    use citysim::components::{Brain, Building, BuildingKind, Job, Lod, Position, ServeKind};
    use citysim::exec::ExecState;
    use citysim::exec::GotoTarget;
    use citysim::goap::LocationKey;
    use citysim::systems::streetwork::{self, RideBy};
    use citysim::systems::{service, streetwork as sw};
    use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY, TICKS_PER_HOUR};

    /// Day 2, 08:00: the trades hired, the commutes about to start.
    const DAY2_EIGHT: u64 = 2 * TICKS_PER_DAY + 8 * TICKS_PER_HOUR;

    fn city_with(edit: impl FnOnce(&mut Config)) -> World {
        let mut w = common::world_with(edit);
        assert!(service::street_on(&w), "the shipped config has the street switch on");
        w.run_ticks(DAY2_EIGHT);
        w
    }

    fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
        w.stats.current.jobs2.clone().unwrap_or_default()
    }

    /// The first Garage with a cab driver and that driver.
    fn garage_and_driver(w: &World) -> (EntityId, EntityId) {
        let role = w.config.trade_role("cab_driver").expect("the cab_driver trade");
        for &g in w.buildings_of_kind(BuildingKind::Garage) {
            if let Some(&d) = w.staff_of(g).iter().find(|&&s| w.comp::<Job>(s).is_some_and(|j| j.role == role)) {
                return (g, d);
            }
        }
        panic!("no Garage with a cab driver on day 2");
    }

    /// A housed day worker who lives `cab_min_tiles` or more from work (the
    /// roll's premise), with the fare and the reserve, not a driver.
    fn far_commuter(w: &World, min: u32) -> Option<EntityId> {
        w.citizens().into_iter().find(|&a| {
            let Some(j) = w.comp::<Job>(a) else { return false };
            let Some(e) = j.employer else { return false };
            let home = w.comp::<citysim::Household>(a).and_then(|h| h.home);
            let (Some(hd), Some(wd)) =
                (home.and_then(|h| w.comp::<Building>(h)).map(|b| b.door), w.comp::<Building>(e).map(|b| b.door))
            else {
                return false;
            };
            hd.manhattan(wd) >= min
                && j.shifts == vec![(540, 1080)]
                && w.has::<Brain>(a)
                && !w.has::<citysim::Sentence>(a)
                && w.gang_of(a).is_none()
                && !citysim::systems::trades::row_of(w, j.role)
                    .is_some_and(|r| r.duty == citysim::config::TradeDuty::Fare)
        })
    }

    /// § 2.3 (cab): a Full commuter rolls the same hash and gets the same pick
    /// as a Statistical one: `cab_roll` is tier-blind, so the same agent on the
    /// same day reads the same `Ride` whatever its `Lod`; `stat_cab` is that
    /// roll plus `pay_ride`.
    #[test]
    fn test_full_commuter_rolls_the_same_cab_as_a_statistical_one() {
        let mut w = city_with(|c| c.jobs2.p_cab = 1.0);
        let min = w.config.jobs2.cab_min_tiles;
        let a = far_commuter(&w, min).expect("a far commuter");
        set_coins(&mut w, a, 500);
        let job = w.comp::<Job>(a).cloned().expect("job");
        w.comp_mut::<Brain>(a).expect("brain").lod = Lod::Full;
        let full = sw::cab_roll(&w, a, &job).expect("a ride at p_cab = 1");
        w.comp_mut::<Brain>(a).expect("brain").lod = Lod::Statistical;
        let stat = sw::cab_roll(&w, a, &job).expect("a ride at p_cab = 1");
        assert_eq!(full, stat, "one roll for both tiers");
        assert!(full.fare >= w.config.jobs2.fare_base);
        // The Statistical path is the roll plus the payment: the same coins move.
        let (before, j0) = (coins(&w, a), jobs2(&w));
        let paid = sw::pay_ride(&mut w, a, &stat);
        assert_eq!(paid, stat.fare);
        assert_eq!(coins(&w, a), before - stat.fare);
        let j = jobs2(&w);
        match stat.by {
            RideBy::Cab(_) => {
                assert_eq!((j.cab_trips - j0.cab_trips, j.cab_fares - j0.cab_fares), (1, stat.fare))
            }
            RideBy::Rickshaw(_) => {
                assert_eq!((j.rickshaw_trips - j0.rickshaw_trips, j.rickshaw_fares - j0.rickshaw_fares), (1, stat.fare))
            }
        }
        // `p_cab = 0`: no roll for either tier.
        let w0 = city_with(|c| c.jobs2.p_cab = 0.0);
        let b = far_commuter(&w0, min).expect("a far commuter");
        assert!(sw::cab_roll(&w0, b, &w0.comp::<Job>(b).cloned().expect("job")).is_none());
    }

    /// § 2.3 (cab, the driver's body): a Full commuter's GoTo to work at
    /// `p_cab = 1` is a cab: the fare paid, the leg timed, and with a driver's
    /// body on duty within `cab_wait_ticks` a `CabRide` the driver's `fare_plan`
    /// drives (the pick-up a `Fare` encounter); the Garage's off-screen cab
    /// takes it otherwise (`cab_rides_unmanned`). One roll a day a rider.
    #[test]
    fn test_full_commuters_cab_is_timed_and_a_driver_drives_it() {
        let mut w = city_with(|c| {
            c.jobs2.p_cab = 1.0;
            c.service.cab_wait_ticks = 100_000;
        });
        let min = w.config.jobs2.cab_min_tiles;
        let (garage, driver) = garage_and_driver(&w);
        // The rider: a far commuter of a Garage's catchment (any: the Garage
        // nearest home is picked, the one with drivers and rides left).
        let a = far_commuter(&w, min).expect("a far commuter");
        set_coins(&mut w, a, 500);
        w.comp_mut::<Brain>(a).expect("brain").lod = Lod::Full;
        // A lawful rider pays (the El's fare rule: a lawless one dodges).
        w.comp_mut::<citysim::Personality>(a).expect("personality").lawfulness = 0.9;
        // The driver on duty all day, a Full body at his Garage.
        if let Some(j) = w.comp_mut::<Job>(driver) {
            j.shifts = vec![(1, 1440)];
        }
        w.comp_mut::<Brain>(driver).expect("brain").lod = Lod::Full;
        w.enter_building(driver, garage);
        // Every other cab driver off duty, so the pick is this one.
        let role = w.config.trade_role("cab_driver").expect("trade");
        for d in w.citizens() {
            if d != driver && w.comp::<Job>(d).is_some_and(|j| j.role == role) {
                if let Some(j) = w.comp_mut::<Job>(d) {
                    j.shifts = vec![(0, 1)];
                }
            }
        }
        let work = w.comp::<Job>(a).and_then(|j| j.employer).expect("work");
        let door = w.comp::<Building>(work).expect("work").door;
        let target = GotoTarget { dest: LocationKey::Workplace, tile: door, building: Some(work) };
        let before = coins(&w, a);
        // The city's own Full riders rode today already: read the deltas.
        let j0 = jobs2(&w);
        let state = sw::full_cab(&mut w, a, &target).expect("a cab at p_cab = 1");
        let ExecState::GotoTimed { arrive_tick, .. } = state else { panic!("a timed leg, not {state:?}") };
        assert!(arrive_tick > w.tick, "the ride takes time");
        assert!(coins(&w, a) < before, "the fare paid");
        let j = jobs2(&w);
        let rides = |j: &citysim::stats::Jobs2Cols| j.cab_rides_driven + j.cab_rides_unmanned + j.rickshaw_trips;
        assert_eq!(rides(&j) - rides(&j0), 1, "one ride");
        assert_eq!(j.fares_short, j0.fares_short, "a lawful rider pays");
        if j.cab_rides_driven > j0.cab_rides_driven {
            assert!(sw::on_fare(&w, driver), "the driver holds the ride");
            assert_eq!(
                sw::fare_tile(&w, driver),
                Some(w.comp::<Position>(a).expect("pos").tile).or(sw::fare_tile(&w, driver))
            );
            assert!(
                j.served[ServeKind::Fare as usize] > j0.served[ServeKind::Fare as usize],
                "the pick-up is a Fare encounter"
            );
            assert!(service::regulars_of(&w, driver).iter().any(|r| r.who == a), "the driver's ledger names the rider");
            // The driver's plan: a leg to the rider's door, then to the work door.
            let p = sw::fare_plan(&mut w, driver).expect("a fare plan");
            assert_eq!(p.steps.len(), 1);
            let from = sw::fare_tile(&w, driver).expect("the leg's tile");
            // Stand him at the rider's door: the next leg is the work door.
            w.leave_building(driver);
            w.comp_mut::<Position>(driver).expect("pos").tile = from;
            sw::fare_plan(&mut w, driver).expect("the second leg");
            assert_eq!(sw::fare_tile(&w, driver), Some(door));
            w.comp_mut::<Position>(driver).expect("pos").tile = door;
            // The ride done: he stands by at the Garage (a GoTo back, a Rest).
            let back = sw::fare_plan(&mut w, driver).expect("the stand-by");
            assert!(!sw::on_fare(&w, driver));
            assert_eq!(back.steps[0].target, Some(garage));
            assert!(sw::standing_by(&w, driver));
        }
        // One roll a day a rider.
        assert!(sw::full_cab(&mut w, a, &target).is_none(), "no second roll today");
        // A rider the El's fare rule says dodges rides short.
        let mut w2 = city_with(|c| {
            c.jobs2.p_cab = 1.0;
            c.transit.dodge_always = 1.0;
        });
        let b = far_commuter(&w2, min).expect("a far commuter");
        set_coins(&mut w2, b, 500);
        w2.comp_mut::<Brain>(b).expect("brain").lod = Lod::Full;
        let work2 = w2.comp::<Job>(b).and_then(|j| j.employer).expect("work");
        let door2 = w2.comp::<Building>(work2).expect("work").door;
        let t2 = GotoTarget { dest: LocationKey::Workplace, tile: door2, building: Some(work2) };
        let (before, short0) = (coins(&w2, b), jobs2(&w2).fares_short);
        sw::full_cab(&mut w2, b, &t2).expect("a cab");
        assert_eq!(coins(&w2, b), before, "a dodged fare moves no coins");
        assert_eq!(jobs2(&w2).fares_short, short0 + 1);
        assert!(w2.events.iter().any(|e| e.kind == EventKind::FareShort && e.actors[0] == b));
    }

    /// The off city (`[service] street = false`): no street row, no street key,
    /// no counter written in three days; the cab roll still runs for the
    /// Statistical tier as before (Jobs & lives phase 2's `cab_trips`).
    #[test]
    fn test_off_city_writes_nothing() {
        let mut off = common::world_with(|c| c.strip_street());
        assert!(!service::street_on(&off));
        assert!(off.config.serves.iter().all(|r| !r.street) && off.config.service.cab_wait_ticks == 0);
        assert!(!off.config.stims.deal_corner);
        off.run_ticks(3 * TICKS_PER_DAY);
        for r in &off.stats.history {
            let j = r.jobs2.clone().unwrap_or_default();
            assert_eq!(
                (j.cab_rides_driven, j.cab_rides_unmanned, j.fares_short, j.parcels_delivered, j.lifts),
                (0, 0, 0, 0, 0)
            );
            assert_eq!((j.served[ServeKind::Fare as usize], j.served[ServeKind::Parcel as usize]), (0, 0));
            // Phase 4b: the pimp's round and cut, the corner, the regulars, the lookout.
            assert_eq!(
                (j.pimp_collections, j.pimp_pass_up, j.corner_deals, j.dealer_regulars, j.lookout_halved),
                (0, 0, 0, 0, 0)
            );
        }
        assert!(off.jobs2.cab_rides.is_empty() && off.jobs2.cab_rolled.is_empty() && off.jobs2.parcels.is_empty());
        assert!(off.jobs2.vice.pimp_seen.is_empty() && off.corner_dealers.is_empty() && off.corner_log.is_empty());
        let ron = citysim::save::to_ron(&off);
        assert!(!ron.contains("cab_wait_ticks") && !ron.contains("cab_rides:") && !ron.contains("deal_corner"));
        assert!(!ron.contains("pimp_seen") && !ron.contains("corner_dealers") && !ron.contains("corner_log"));
        let _ = streetwork::on(&off);
    }

    /// A Freight Yard's courier (a `round` duty at the Yard).
    fn yard_and_courier(w: &World) -> (EntityId, EntityId) {
        for &y in w.buildings_of_kind(BuildingKind::FreightYard) {
            let c = w.staff_of(y).iter().copied().find(|&s| citysim::systems::trades::on_round(w, s));
            if let Some(c) = c {
                return (y, c);
            }
        }
        panic!("no Freight Yard with a courier on day 2");
    }

    /// A building other than `not` with a staff member made present: a Full
    /// body inside on an all-day shift (the first for which `present` holds).
    fn staffed_with_present(w: &mut World, not: EntityId) -> (EntityId, EntityId) {
        let cands: Vec<(EntityId, EntityId)> = w
            .citizens()
            .into_iter()
            .filter_map(|s| Some((w.comp::<Job>(s)?.employer?, s)))
            .filter(|&(b, s)| {
                b != not
                    && w.has::<Brain>(s)
                    && !w.has::<citysim::Sentence>(s)
                    && w.comp::<Building>(b).is_some_and(|x| x.kind != BuildingKind::FreightYard)
            })
            .collect();
        for (b, s) in cands {
            if let Some(j) = w.comp_mut::<Job>(s) {
                j.shifts = vec![(1, 1440)];
            }
            citysim::systems::lod::set_lod(w, s, Lod::Full);
            w.enter_building(s, b);
            if service::present(w, s, b) {
                return (b, s);
            }
        }
        panic!("no staff member could be made present");
    }

    /// § 2.3 (courier): a parcel is handed to its recipient present at the
    /// stop (a `Parcel` encounter naming him); with nobody present it stays,
    /// and at the courier's shift end it goes back to the Yard.
    #[test]
    fn test_parcel_delivered_to_a_present_recipient_and_returned_otherwise() {
        let mut w = city_with(|_| {});
        let (yard, courier) = yard_and_courier(&w);
        w.jobs2.parcels.clear();
        let (b, r) = staffed_with_present(&mut w, yard);
        let day = w.day();
        let parcel = |to: EntityId, at: EntityId| sw::Parcel { from: yard, to, at, yard, fee: 0, day };
        w.jobs2.parcels.push(parcel(r, b));
        let j0 = jobs2(&w);
        sw::courier_stop(&mut w, courier, b);
        let j = jobs2(&w);
        assert_eq!(j.parcels_delivered - j0.parcels_delivered, 1, "delivered");
        assert!(w.jobs2.parcels.is_empty());
        assert!(j.served[ServeKind::Parcel as usize] > j0.served[ServeKind::Parcel as usize], "a Parcel encounter");
        assert!(service::regulars_of(&w, courier).iter().any(|x| x.who == r), "the courier's ledger names him");
        // Nobody present at the stop: every staff member a Full body outside.
        let staff = w.staff_of(b).to_vec();
        for &s in &staff {
            citysim::systems::lod::set_lod(&mut w, s, Lod::Full);
            w.leave_building(s);
        }
        assert!(staff.iter().all(|&s| !service::present(&w, s, b)));
        w.jobs2.parcels.push(parcel(r, b));
        sw::courier_stop(&mut w, courier, b);
        assert_eq!(w.jobs2.parcels.len(), 1, "no one to sign: the parcel stays");
        assert_eq!(jobs2(&w).parcels_delivered, j.parcels_delivered);
        // The shift's end: the stop was on his round, so it goes back.
        w.jobs2.parcel_stops.insert(yard, vec![b]);
        sw::parcels_back(&mut w, courier);
        assert!(w.jobs2.parcels.is_empty(), "returned");
        assert_eq!(jobs2(&w).parcels_returned - j0.parcels_returned, 1);
    }

    /// The loaders' pilfer through the first night's end (06:00 on day 3),
    /// with `p_pilfer` 1: (pilfered, pilfer_reports, Theft reports on a
    /// pilferer).
    fn pilfer_night(p_short: f32) -> (u32, u32, usize) {
        let mut w = city_with(|c| {
            c.jobs2.p_pilfer = 1.0;
            c.service.p_short_report = p_short;
        });
        w.run_ticks(23 * TICKS_PER_HOUR);
        let sum = |f: fn(&citysim::stats::Jobs2Cols) -> u32| -> u32 {
            w.stats.history.iter().chain(std::iter::once(&w.stats.current)).map(|r| r.jobs2.as_ref().map_or(0, f)).sum()
        };
        let thieves: Vec<EntityId> =
            w.events.iter().filter(|e| e.kind == EventKind::Pilfered).map(|e| e.actors[0]).collect();
        let reports = w
            .events
            .iter()
            .filter(|e| e.kind == EventKind::Report && thieves.contains(&e.actors[0]) && e.text.contains("Theft"))
            .count();
        (sum(|j| j.pilfered), sum(|j| j.pilfer_reports), reports)
    }

    /// § 2.3 (loader): a pilfered unit files a Theft report against the
    /// loader when the owner's hashed roll hits (`p_short_report` 1: every
    /// one), and never at `p_short_report` 0.
    #[test]
    fn test_pilfered_unit_files_a_theft_report_on_the_hashed_hit_and_never_at_zero() {
        let (pilfered, filed, reports) = pilfer_night(1.0);
        assert!(pilfered > 0, "a pilfer at p_pilfer 1");
        assert_eq!(filed, pilfered, "every pilfer reported at 1");
        assert!(reports > 0, "a Theft report names a pilferer");
        let (pilfered, filed, reports) = pilfer_night(0.0);
        assert!(pilfered > 0);
        assert_eq!((filed, reports), (0, 0), "never at 0");
    }

    /// § 2.3 (loader): the shift's lifts are the loader's at its last hour
    /// (`service::hourly`, both tiers through `present`): `lifts` and
    /// `served[Crate]` by the Yard's units over its loaders on shift, one
    /// `Served` memory with no subject; none an hour earlier.
    #[test]
    fn test_loader_lifts_the_yards_units_at_the_shifts_last_hour() {
        let mut w = city_with(|_| {});
        let crate_row = |w: &World, s: EntityId| {
            w.comp::<Job>(s).and_then(|j| service::row_for(w, j.role)).is_some_and(|r| r.kind == ServeKind::Crate)
        };
        let (yard, loader) = w
            .buildings_of_kind(BuildingKind::FreightYard)
            .iter()
            .find_map(|&y| w.staff_of(y).iter().copied().find(|&s| crate_row(&w, s)).map(|s| (y, s)))
            .expect("a loader");
        // Day 2, 08:00: his shift runs to 10:00, then to 09:00.
        if let Some(j) = w.comp_mut::<Job>(loader) {
            j.shifts = vec![(60, 600)];
        }
        citysim::systems::lod::set_lod(&mut w, loader, Lod::Full);
        w.enter_building(loader, yard);
        assert!(service::present(&w, loader, yard));
        w.jobs2.yard_units.insert(yard, (40, 0));
        let n = citysim::systems::industry::shift_lifts(&w, yard);
        assert!(n >= 1);
        let served = |w: &World| {
            let m = w.comp::<citysim::components::Memory>(loader).expect("memory");
            m.entries.iter().any(|e| e.kind == citysim::components::MemoryKind::Served && e.subject.is_none())
        };
        let j0 = jobs2(&w);
        service::hourly(&mut w);
        assert!(!served(&w), "not before the shift's last hour");
        if let Some(j) = w.comp_mut::<Job>(loader) {
            j.shifts = vec![(60, 540)];
        }
        service::hourly(&mut w);
        let j = jobs2(&w);
        assert!(j.lifts - j0.lifts >= n, "his lifts counted");
        assert!(j.served[ServeKind::Crate as usize] - j0.served[ServeKind::Crate as usize] >= n);
        assert!(served(&w), "the lift's memory, no subject");
    }

    /// The street test's night: a working street pitch with no pimp, the
    /// visit and assault rolls certain, and a lawless Full client on its
    /// tile, every other body near it wanting nothing. (world, worker, client)
    fn street_night(on: bool) -> (World, EntityId, EntityId) {
        let (mut w, o, c) = street_setup(on);
        citysim::systems::vice::hourly(&mut w);
        (w, o, c)
    }

    /// The shipped city at day 1's first tick, built once (the street
    /// night's base; each test clones it).
    static NIGHT: std::sync::OnceLock<World> = std::sync::OnceLock::new();

    /// `street_night` before the hour's pass.
    fn street_setup(on: bool) -> (World, EntityId, EntityId) {
        let mut w = if on {
            NIGHT
                .get_or_init(|| {
                    // Lives at work phase 6: careers and lieutenants off (the
                    // pimp's night needs a free gang member on day 1).
                    let mut w = common::world_with(|c| {
                        c.strip_careers();
                        c.gangs.strip_lieutenants();
                    });
                    w.run_ticks(TICKS_PER_DAY + 1);
                    w
                })
                .clone()
        } else {
            let mut w = common::world_with(|c| c.strip_street());
            w.run_ticks(TICKS_PER_DAY + 1);
            w
        };
        w.config.vice.p_street_visit = 10.0;
        w.config.vice.p_assault_street = 10.0;
        w.jobs2.vice.pimps.clear();
        let idle: Vec<EntityId> = w
            .citizens()
            .into_iter()
            .filter(|&a| {
                w.has::<Brain>(a)
                    && citysim::systems::demography::is_adult(&w, a)
                    && w.gang_of(a).is_none()
                    && !w.has::<Job>(a)
                    && !w.has::<citysim::Sentence>(a)
            })
            .collect();
        let (o, c) = (idle[2], idle[3]);
        citysim::systems::lod::set_lod(&mut w, o, Lod::Statistical);
        let trade = w.config.trade_role("street_worker").and_then(|r| r.trade()).expect("the street row");
        let spot = w.comp::<Position>(o).expect("pos").tile;
        let p = citysim::components::Pitch {
            spot,
            district: w.district_of(spot),
            trade,
            owner: o,
            since: w.tick,
            take_today: 0,
            take_week: 0,
            cut_to: None,
            stock: 0,
            closed_until: None,
            dry_days: 0,
            jobs_today: 0,
        };
        w.jobs2.pitches.insert(o, p.clone());
        citysim::systems::lod::set_lod(&mut w, c, Lod::Full);
        w.leave_building(c);
        w.comp_mut::<Position>(c).expect("pos").tile = spot;
        set_coins(&mut w, c, 200);
        w.comp_mut::<citysim::Personality>(c).expect("personality").lawfulness = 0.1;
        let r = w.config.service.client_tiles.max(12);
        for a in w.agents_near(spot, r, None) {
            if let Some(n) = w.comp_mut::<citysim::Needs>(a) {
                n.intimacy = if a == c { 0.0 } else { 1.0 };
                n.hunger = 1.0;
            }
        }
        w.tick = TICKS_PER_DAY + 2 * 60;
        assert!(sw::working(&w, &p));
        (w, o, c)
    }

    /// The street night with a free pimp of the pitch's district, a body
    /// standing `near` the pitch (on its spot) or across town: the hour's
    /// assaults. (world, pimp, assaults)
    fn pimp_night(near: bool) -> (World, EntityId, u32) {
        let (mut w, o, c) = street_setup(true);
        let p = w.jobs2.pitches.get(&o).cloned().expect("the pitch");
        let pimp = w
            .gangs()
            .into_iter()
            .flat_map(|g| w.comp::<citysim::Gang>(g).map(|x| x.members.clone()).unwrap_or_default())
            .find(|&m| m != o && m != c && citysim::systems::law::living(&w, m) && !w.has::<citysim::Sentence>(m))
            .expect("a gang member");
        w.jobs2.vice.pimps.insert(p.district.index() as u16, pimp);
        citysim::systems::lod::set_lod(&mut w, pimp, Lod::Full);
        w.leave_building(pimp);
        let r = w.config.service.pimp_tiles;
        let far = w
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .filter_map(|&b| w.comp::<Building>(b).map(|x| x.door))
            .find(|d| d.manhattan(p.spot) > 4 * r)
            .expect("a door across town");
        w.comp_mut::<Position>(pimp).expect("pos").tile = if near { p.spot } else { far };
        if let Some(n) = w.comp_mut::<citysim::Needs>(pimp) {
            n.intimacy = 1.0;
        }
        let before = jobs2(&w).street_assaults;
        citysim::systems::vice::hourly(&mut w);
        let n = jobs2(&w).street_assaults - before;
        (w, pimp, n)
    }

    /// § 2.3 (pimp): the street assault reads the pimp's presence, not his
    /// freedom: a free pimp across town leaves the pitch unprotected (the
    /// lawless client robs), one within `pimp_tiles` protects it and is
    /// marked seen (`pimp_seen`, what `name_pimps` drops the absent by).
    #[test]
    fn test_street_assault_reads_the_pimps_presence_not_his_freedom() {
        let (w, _, far) = pimp_night(false);
        assert_eq!(far, 1, "a free pimp across town: unprotected");
        assert!(w.jobs2.vice.pimp_seen.is_empty(), "never seen at his pitch");
        let (w, pimp, near) = pimp_night(true);
        assert_eq!(near, 0, "a pimp within pimp_tiles: protected");
        let d = citysim::systems::vice::pimp_district(&w, pimp).expect("his district");
        assert_eq!(w.jobs2.vice.pimp_seen.get(&(d.index() as u16)), Some(&w.day()), "seen today");
    }

    /// § 2.3 (street worker): the night's client is a body out near the
    /// pitch before the district's hashed draw: the encounter names him (the
    /// worker's ledger), and a lawless client is the one who robs on the
    /// assault roll. Off, no client near is read.
    #[test]
    fn test_street_client_is_a_body_near_the_pitch_and_a_lawless_one_robs() {
        let (w, o, c) = street_night(true);
        let j = jobs2(&w);
        assert_eq!(j.street_clients_near, 1, "the body near the pitch");
        assert_eq!(j.served[ServeKind::Client as usize], 1, "a Client encounter");
        assert!(service::regulars_of(&w, o).iter().any(|x| x.who == c), "the worker's ledger names the client");
        assert!(
            w.events.iter().any(|e| e.kind == EventKind::Assault && e.actors[0] == c && e.actors[1] == o),
            "the lawless client robs"
        );
        let (w, _, _) = street_night(false);
        let j = jobs2(&w);
        assert_eq!((j.street_clients_near, j.served[ServeKind::Client as usize]), (0, 0));
    }

    /// 8d (OPEN-129): an exec housed in the Spire whose Hq stands in
    /// another district is driven to work: the leg timed at the Car's
    /// speed, the corp's own road vehicle or the Garage's off-screen cab on
    /// the corp's purse, once a day; off (`exec_ride` false), he walks.
    fn exec_far_from_hq(w: &mut World) -> (EntityId, EntityId, EntityId) {
        let tier = citysim::systems::life::exec_home_tier(w);
        assert!(tier > 0, "the shipped config houses execs in the Spire");
        for c in w.corps() {
            let Some(e) = w.comp::<citysim::Corp>(c).and_then(|cc| cc.exec) else { continue };
            if w.has::<Job>(e) || w.has::<citysim::Sentence>(e) {
                continue;
            }
            let Some(hq) = citysim::systems::life::hq_of(w, c, e) else { continue };
            let d = w.district_of_building(hq);
            let spire = w.buildings_of_kind(BuildingKind::Home).iter().copied().find(|&h| {
                w.comp::<Building>(h).is_some_and(|b| {
                    b.tier >= tier && !b.demolished && !b.derelict && usize::from(b.capacity) > w.residents_of(h).len()
                }) && w.district_of_building(h) != d
            });
            let Some(home) = spire else { continue };
            w.set_home(e, Some(home));
            // `hq_of` ranks by the distance from home: read it again.
            let hq = citysim::systems::life::hq_of(w, c, e).expect("an Hq");
            if w.district_of_building(hq) == w.district_of_building(home) {
                continue;
            }
            return (e, c, hq);
        }
        panic!("an exec with a Spire Home outside his Hq's district");
    }

    #[test]
    fn test_the_exec_rides_in_from_the_spire() {
        let mut w = city_with(|_| {});
        assert!(w.config.service.exec_ride, "the shipped config drives the execs");
        let (e, corp, hq) = exec_far_from_hq(&mut w);
        let home = citysim::systems::missions::home_of(&w, e).expect("home");
        let hd = w.comp::<Building>(home).expect("home").door;
        w.leave_building(e);
        if let Some(p) = w.comp_mut::<Position>(e) {
            p.tile = hd;
            p.building = None;
        }
        w.jobs2.cab_rolled.clear();
        let door = w.comp::<Building>(hq).expect("hq").door;
        let target = GotoTarget { dest: LocationKey::Workplace, tile: door, building: Some(hq) };
        if let Some(c) = w.comp_mut::<citysim::Corp>(corp) {
            c.treasury = c.treasury.max(10_000);
        }
        let purse = w.purse(Some(corp));
        let j0 = jobs2(&w);
        let state = sw::exec_ride(&mut w, e, &target).expect("the exec rides in");
        let ExecState::GotoTimed { arrive_tick, .. } = state else { panic!("a timed leg, not {state:?}") };
        let walk = u64::from(hd.manhattan(door)) * w.config.exec.move_ticks_full;
        assert!(arrive_tick > w.tick, "the ride takes time");
        assert!(arrive_tick - w.tick < walk, "faster than the walk ({} against {walk})", arrive_tick - w.tick);
        let j = jobs2(&w);
        assert_eq!(j.exec_rides, j0.exec_rides + 1, "one ride counted");
        if j.exec_rides_corp == j0.exec_rides_corp {
            assert!(w.purse(Some(corp)) < purse, "the corp paid the cab");
            assert_eq!(j.cab_rides_unmanned, j0.cab_rides_unmanned + 1, "the Garage's off-screen cab");
        } else {
            assert_eq!(w.purse(Some(corp)), purse, "the corp's own vehicle: no fare");
        }
        assert!(sw::exec_ride(&mut w, e, &target).is_none(), "once a day");
        // Off: he walks.
        w.jobs2.cab_rolled.clear();
        w.config.service.exec_ride = false;
        assert!(sw::exec_ride(&mut w, e, &target).is_none(), "off, no ride");
    }

    /// Review fix 2: a booked `CabRide` belongs to a driver who can still
    /// drive it. Off his shift the ride is not his (`on_fare` false, no
    /// fare tile for `leisure::spot_of`) and the hour drops it; on shift
    /// with another goal's plan, `spot_of` is that goal's spot, not the
    /// fare's. Failed on the base `f795548` (the ride stuck to him till
    /// midnight).
    #[test]
    fn test_a_stale_cab_ride_is_dropped() {
        let mut w = city_with(|_| {});
        let (_, driver) = garage_and_driver(&w);
        let rider = w.citizens().into_iter().find(|&a| a != driver).expect("a rider");
        let (from, to) = (citysim::TilePos { x: 3, y: 3 }, citysim::TilePos { x: 5, y: 5 });
        let ride = sw::CabRide { rider, from, to, driver, fare: 4, tick: w.tick, stage: 0 };
        // On shift all day: the ride is his, and the Work plan's spot is the fare.
        if let Some(j) = w.comp_mut::<Job>(driver) {
            j.shifts = vec![(1, 1440)];
        }
        w.jobs2.cab_rides.push(ride);
        assert!(sw::on_fare(&w, driver));
        assert_eq!(sw::fare_tile(&w, driver), Some(from));
        // Another goal's plan: its spot, not the fare's.
        w.comp_mut::<Brain>(driver).expect("brain").plan = Some(citysim::goap::Plan {
            goal: citysim::components::GoalKind::Socialise,
            target: None,
            steps: vec![],
            started_tick: w.tick,
        });
        assert_ne!(citysim::systems::leisure::spot_of(&w, driver), Some(from), "a pre-empted plan is not the fare's");
        // The shift over: not his ride, and the hour drops it.
        if let Some(j) = w.comp_mut::<Job>(driver) {
            j.shifts = vec![(0, 1)];
        }
        assert!(!sw::on_fare(&w, driver), "off shift, the ride is not his");
        assert_eq!(sw::fare_tile(&w, driver), None);
        assert!(w.tick.is_multiple_of(TICKS_PER_HOUR));
        sw::hourly(&mut w);
        assert!(w.jobs2.cab_rides.iter().all(|r| r.driver != driver), "the hour drops the stale ride");
    }
}
