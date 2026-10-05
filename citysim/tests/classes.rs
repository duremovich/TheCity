//! M11 phase 4: founding, incorporation and classes (docs/M11_OWNERSHIP.md
//! § 6, § 7, § 11; plan phase 4 tests, D25-D27, D34-D36).

use citysim::systems::{classes, founding, gang, law, ownership};
use citysim::{
    ActionKind, Brain, Building, BuildingKind, Class, Config, Corp, CorpShock, EntityId, EventKind, GangMember,
    GoalKind, Household, Job, LifeKind, LocationKey, Mood, PlanCtx, Position, Role, TileKind, TilePos, Wallet, World,
    WorldState, TICKS_PER_DAY, TICKS_PER_HOUR,
};

/// A small v2 city: Lots, the eight seeded corps.
fn v2_world(seed: u64) -> World {
    World::new(seed, Config::load().scaled_to(400))
}

/// The v1 city with two corps (FoodCo: both Farms, the Market, the Bar;
/// HomeCo: 30 Blocks) and the classes on.
fn v1_corp_world() -> World {
    let mut c = Config::load().v1_profile();
    c.corps.names = vec!["FoodCo".into(), "HomeCo".into()];
    c.corps.niches = vec![vec!["Food".into()], vec!["Housing".into()]];
    c.corps.farms = vec![2, 0];
    c.corps.markets = vec![1, 0];
    c.corps.blocks = vec![0, 30];
    c.corps.offices = vec![0, 0];
    c.corps.treasury_initial = vec![2000, 2000];
    c.classes = Config::load().classes;
    World::new(7, c)
}

fn coins(w: &World, a: EntityId) -> i64 {
    w.comp::<Wallet>(a).map_or(0, |w| w.coins)
}

/// A jobless, housed adult outside every gang who is nobody's exec.
fn founder(w: &World) -> EntityId {
    w.citizens()
        .into_iter()
        .find(|&a| {
            w.has::<Brain>(a)
                && !w.has::<Job>(a)
                && !w.has::<GangMember>(a)
                && citysim::systems::demography::is_adult(w, a)
                && !founding::is_exec(w, a)
                && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
        })
        .expect("a founder")
}

fn count(w: &World, kind: EventKind) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

fn set_mood(w: &mut World, a: EntityId, v: f32) {
    w.comp_mut::<Mood>(a).expect("mood").value = v;
}

// ---------------------------------------------------------------------------
// Founding
// ---------------------------------------------------------------------------

#[test]
fn test_register_converts_lot_and_stamps_walls() {
    let mut w = v2_world(11);
    let a = founder(&w);
    w.comp_mut::<Wallet>(a).expect("w").coins = 500;
    let lots0 = w.buildings_of_kind(BuildingKind::Lot).len();
    let treasury0 = w.purse(None);
    // Cache a flow field to every door, as walkers would have.
    for b in w.with::<Building>() {
        let door = w.comp::<Building>(b).expect("b").door;
        let field = citysim::exec::flowfield::FlowField::build(&w.map, door);
        w.flow_fields.insert(b, field, 100_000);
    }
    let cached0 = w.flow_fields.len();
    assert!(founding::can_found(&w, a));
    let kind = founding::choose_kind(&w, 500).expect("a kind");
    assert_eq!(kind, BuildingKind::Bar, "far fewer Bars per head than Blocks");
    let lot = founding::register(&mut w, a).expect("registered");
    let cost = w.config.corps.found_cost.bar;
    let b = w.comp::<Building>(lot).expect("b").clone();
    assert_eq!(b.kind, BuildingKind::Bar);
    assert_eq!(b.owner, Some(a), "the founder owns it");
    assert_eq!(w.buildings_of_kind(BuildingKind::Lot).len(), lots0 - 1);
    for y in b.rect.y..b.rect.y + b.rect.h {
        for x in b.rect.x..b.rect.x + b.rect.w {
            let t = TilePos { x, y };
            if b.rect.on_perimeter(t) {
                let want = if t == b.door { TileKind::Door } else { TileKind::Wall };
                assert_eq!(w.map.tile_at(t), want, "perimeter at {t:?}");
            }
        }
    }
    assert!(w.vacancies.get(&lot).is_some_and(|v| v.contains(&Role::Bartender)), "the Bar posts a bartender");
    assert_eq!(coins(&w, a), 500 - cost);
    assert_eq!(w.purse(None), treasury0 + cost, "found_cost goes to the Treasury");
    assert!(w.flow_fields.len() < cached0, "fields whose paths crossed the Lot are dropped");
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Founded).expect("Founded");
    assert_eq!(e.actors.first(), Some(&a), "the founder in slot 0");
    assert!(e.text.contains("registered"), "{}", e.text);
    assert!(
        w.comp::<citysim::Life>(a).is_some_and(|l| l.events.iter().any(|e| e.kind == LifeKind::Founded)),
        "a Life row"
    );
    // Cooldown: no second founding the same week.
    assert!(!founding::can_found(&w, a));
    assert!(founding::register(&mut w, a).is_err());
}

#[test]
fn test_owning_two_buildings_incorporates() {
    let mut w = v2_world(12);
    let a = founder(&w);
    w.comp_mut::<Wallet>(a).expect("w").coins = 1000;
    let first = founding::register(&mut w, a).expect("first");
    assert_eq!(count(&w, EventKind::Incorporated), 0, "one building is not a company");
    w.comp_mut::<Brain>(a).expect("b").last_found_day = None;
    let before = coins(&w, a);
    let second = founding::register(&mut w, a).expect("second");
    let cost = before - coins(&w, a) - w.purse(Some(w.corps().last().copied().expect("corp")));
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Incorporated).expect("Incorporated").clone();
    let corp = e.actors[0];
    assert_eq!(e.actors.get(1), Some(&a), "the exec in slot 1");
    let c = w.comp::<Corp>(corp).expect("a corp").clone();
    assert_eq!(c.exec, Some(a));
    assert_eq!(c.slot, None);
    // Phase 5: the founder's savings, less a day's exec wage, are the capital.
    let float = w.config.economy.wage_exec;
    assert_eq!(coins(&w, a), float, "the founder keeps a day's wage");
    assert_eq!(c.treasury, before - cost - float, "the rest is the corp's");
    assert!(cost > 0);
    let grace = w.config.corps.incorporate_grace_days;
    assert_eq!(c.upkeep_grace_until, (grace > 0).then(|| w.tick + grace * TICKS_PER_DAY));
    assert!(c.name.ends_with(" Holdings"), "{}", c.name);
    assert_eq!(c.buildings, {
        let mut v = vec![first, second];
        v.sort();
        v
    });
    for b in [first, second] {
        assert_eq!(w.comp::<Building>(b).expect("b").owner, Some(corp));
    }
    assert!(w.comp::<citysim::Life>(a).is_some_and(|l| l.events.iter().any(|e| e.kind == LifeKind::Incorporated)));
    // Revenue now lands in the corp's treasury.
    let t0 = w.purse(Some(corp));
    let payer = founder_other(&w, a);
    ownership::pay(&mut w, Some(payer), Some(corp), 2, ownership::Flow::Drink);
    assert!(w.purse(Some(corp)) > t0, "drink money goes to the corp");
    // The exec draws a wage at the next midnight.
    ownership::charge(&mut w, None, Some(corp), 500, ownership::Flow::Subsidy);
    let wallet0 = coins(&w, a);
    w.tick = TICKS_PER_DAY;
    ownership::run(&mut w);
    assert!(coins(&w, a) > wallet0, "exec wage paid from the corp");
}

fn founder_other(w: &World, not: EntityId) -> EntityId {
    w.citizens().into_iter().find(|&x| x != not && w.has::<Brain>(x) && coins(w, x) >= 2).expect("a payer")
}

#[test]
fn test_found_goal_plans_hall_then_register() {
    let mut w = v2_world(13);
    let a = founder(&w);
    w.comp_mut::<Wallet>(a).expect("w").coins = 500;
    // Out on the street, not at the Hall.
    w.leave_building(a);
    let ctx = PlanCtx::build(&w, a, None);
    assert!(ctx.can_found);
    let start = WorldState::observe(&w, a, None);
    let goal = citysim::goap::goal_state(GoalKind::Found).expect("goal");
    let limits = citysim::goap::Limits { max_expansions: 400, max_len: 6 };
    let steps: Vec<ActionKind> = citysim::goap::planner::plan(&ctx, start, &goal, limits).expect("a plan").steps;
    assert_eq!(steps, vec![ActionKind::GoTo(LocationKey::Hall), ActionKind::Register]);
    // The utility side: the goal is scored only while founding is possible.
    assert!(!citysim::utility::goals::already_satisfied(&w, a, GoalKind::Found, false));
    w.comp_mut::<Wallet>(a).expect("w").coins = 10;
    assert!(citysim::utility::goals::already_satisfied(&w, a, GoalKind::Found, false));
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

#[test]
fn test_class_aggregates_on_hand_built_world() {
    let mut w = v1_corp_world();
    let food = w.corps()[0];
    // A FoodCo farmer (Corp), a housed jobless agent on the dole (Street),
    // a homeless jobless agent (Dreg).
    let worker = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(food))
        .expect("a FoodCo farmer");
    let street = founder(&w);
    let dreg = w
        .citizens()
        .into_iter()
        .find(|&x| {
            x != street
                && w.has::<Brain>(x)
                && !w.has::<Job>(x)
                && citysim::systems::demography::is_adult(&w, x)
                && !founding::is_exec(&w, x)
        })
        .expect("another jobless adult");
    w.set_home(dreg, None);
    set_mood(&mut w, worker, 0.6);
    set_mood(&mut w, street, 0.0);
    set_mood(&mut w, dreg, -0.6);
    assert_eq!(classes::class_of(&w, worker), Class::Corp);
    assert_eq!(classes::class_of(&w, street), Class::Street);
    assert_eq!(classes::class_of(&w, dreg), Class::Dreg);
    // One guard parked 3 tiles from the Street agent's Home door for 24 hours.
    let home = w.comp::<Household>(street).and_then(|h| h.home).expect("home");
    let door = w.comp::<Building>(home).expect("b").door;
    let guard = w.workers(Role::Guard)[0];
    {
        let j = w.comp_mut::<Job>(guard).expect("job");
        j.shifts = vec![(0, 1440)];
    }
    w.leave_building(guard);
    w.comp_mut::<Position>(guard).expect("p").tile = TilePos { x: door.x + 3, y: door.y };
    w.home_watch = Default::default();
    for h in 0..24u64 {
        w.tick = h * TICKS_PER_HOUR;
        let tod = w.tick_of_day();
        law::tally_home_watch(&mut w, tod);
    }
    assert!(w.home_watch.today.get(&home).copied().unwrap_or(0) >= 24, "a guard-hour per hour");
    w.home_watch.yesterday = std::mem::take(&mut w.home_watch.today);
    w.home_watch.yesterday.retain(|&h, _| h == home);
    let (_, _, _, fear_street) = classes::member_inputs(&w, street);
    assert!((fear_street - 1.0).abs() < 1e-6, "24 guard-hours read as full fear (clamped)");
    let (_, _, _, fear_worker) = classes::member_inputs(&w, worker);
    let worker_home = w.comp::<Household>(worker).and_then(|h| h.home);
    if worker_home != Some(home) {
        assert_eq!(fear_worker, 0.0, "no guard near the worker's Home");
    }
    // The documented formulas, one member each.
    let agg = |a: EntityId, ev: u32| {
        let (_, m, e, f) = classes::member_inputs(&w, a);
        classes::aggregate(&[(m, e, f)], ev)
    };
    let corp = agg(worker, 0);
    let st = agg(street, 2);
    let dr = agg(dreg, 0);
    let close = |x: f32, y: f32| assert!((x - y).abs() < 1e-5, "{x} vs {y}");
    close(corp.happiness, 0.8);
    close(st.happiness, 0.5);
    close(dr.happiness, 0.2);
    close(corp.employment, 1.0);
    close(st.employment, 0.0);
    close(corp.loyalty, 0.8);
    close(st.loyalty, 0.5 * 0.5);
    close(dr.loyalty, 0.2 * 0.5);
    close(st.fear, 1.0);
    close(st.submission, 1.0);
    close(corp.submission, 0.3 + 0.7 * fear_worker);
    close(st.unrest, (1.0 - 0.25) * 0.0 + 0.1 * 2.0);
    close(corp.unrest, (1.0 - 0.8) * (1.0 - corp.submission));
    close(dr.unrest, (1.0 - 0.1) * (1.0 - dr.submission));
    assert_eq!(st.count, 1);
    assert_eq!(st.evictions_7d, 2);
    // The daily pass fills World::classes for the whole city.
    w.tick = 2 * TICKS_PER_DAY;
    classes::compute(&mut w);
    let total: u32 = w.classes.iter().map(|c| c.count).sum();
    assert!(total > 0 && w.classes[Class::Dreg.index()].count >= 1);
}

#[test]
fn test_strike_skips_shift_and_shocks_corp() {
    let mut w = v1_corp_world();
    let food = w.corps()[0];
    w.tick = 3 * TICKS_PER_DAY;
    // FoodCo charges the most.
    w.comp_mut::<Corp>(food).expect("c").price_level.insert(citysim::Niche::Food, 1.4);
    w.classes[Class::Street.index()].unrest = w.config.classes.strike_threshold - 0.01;
    classes::strike(&mut w);
    assert_eq!(count(&w, EventKind::Strike), 0, "below the threshold");
    w.classes[Class::Street.index()].unrest = 0.9;
    let (target, strikers) = classes::strike_target(&w).expect("a target");
    assert_eq!(target, food);
    assert!(!strikers.is_empty());
    classes::strike(&mut w);
    assert_eq!(count(&w, EventKind::Strike), 1);
    let tick = w.tick;
    for &s in &strikers {
        let j = w.comp::<Job>(s).expect("job");
        assert_eq!(j.last_shift_day, Some(j.next_shift_key(tick)), "the next shift is struck");
        assert!(citysim::utility::goals::already_satisfied(&w, s, GoalKind::Work, false), "nothing to work");
    }
    assert!(w.comp::<Corp>(food).expect("c").shocks.contains(&CorpShock::Strike));
    assert_eq!(w.stats.current.strikes, 1);
    // The lockout: not again within strike_cooldown_days.
    w.tick += TICKS_PER_DAY;
    classes::strike(&mut w);
    assert_eq!(count(&w, EventKind::Strike), 1, "locked out");
    w.tick += w.config.classes.strike_cooldown_days * TICKS_PER_DAY;
    classes::strike(&mut w);
    assert_eq!(count(&w, EventKind::Strike), 2, "after the lockout");
}

#[test]
fn test_immigration_scales_with_street_happiness() {
    let mut w = v1_corp_world();
    w.levers.immigration_per_week = 10;
    let s = Class::Street.index();
    w.classes[s].happiness = 0.2;
    let low = classes::immigrants_this_week(&w);
    w.classes[s].happiness = 0.8;
    let high = classes::immigrants_this_week(&w);
    assert!(low < high, "{low} vs {high}");
    assert!(high <= 10, "the lever is the ceiling");
    assert!(classes::immigration_factor(&w) > 0.9);
    w.classes[s].happiness = 0.2;
    assert!(classes::immigration_factor(&w) < 0.1);
    // v1_profile: the lever, uncoupled.
    let mut v1 = World::new(7, Config::load().v1_profile());
    v1.levers.immigration_per_week = 10;
    v1.classes[s].happiness = 0.2;
    assert_eq!(classes::immigrants_this_week(&v1), 10);
}

#[test]
fn test_dreg_emigrates_after_seven_miserable_days_and_corp_class_never_does() {
    let mut w = v1_corp_world();
    let food = w.corps()[0];
    let worker = w
        .workers(Role::Farmer)
        .iter()
        .copied()
        .find(|&f| w.comp::<Job>(f).and_then(|j| j.employer).and_then(|e| w.owner_of(e)) == Some(food))
        .expect("a FoodCo farmer");
    let dreg = founder(&w);
    w.set_home(dreg, None);
    w.set_home(worker, None);
    let days = w.config.classes.dreg_emigrate_days;
    assert!(days > 0);
    w.tick = TICKS_PER_DAY;
    for d in 0..days {
        set_mood(&mut w, dreg, -0.9);
        set_mood(&mut w, worker, -0.9);
        assert!(!w.comp::<Brain>(dreg).expect("b").emigrating, "not before day {d}");
        classes::run(&mut w);
        w.tick += TICKS_PER_DAY;
    }
    assert!(w.comp::<Brain>(dreg).expect("b").emigrating, "a week miserable on the street");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Emigration && e.actors.first() == Some(&dreg)));
    assert_eq!(classes::class_of(&w, worker), Class::Corp);
    assert!(!w.comp::<Brain>(worker).expect("b").emigrating, "a corp worker never leaves");
    assert_eq!(w.comp::<Household>(worker).expect("h").miserable_days, 0);
    // Nor by the general mood rule.
    {
        let m = w.comp_mut::<Mood>(worker).expect("m");
        m.value = -0.95;
        m.low_since = Some(0);
    }
    w.tick = 30 * TICKS_PER_DAY;
    citysim::systems::demography::run(&mut w);
    assert!(!w.comp::<Brain>(worker).expect("b").emigrating, "Corp class never emigrates");
}

#[test]
fn test_evicted_lawless_agent_is_recruitable() {
    let mut w = v1_corp_world();
    w.tick = 5 * TICKS_PER_DAY;
    let candidates: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&x| {
            w.has::<Brain>(x)
                && !w.has::<GangMember>(x)
                && citysim::systems::demography::is_adult(&w, x)
                && w.comp::<Job>(x).is_none_or(|j| j.role != Role::Guard)
        })
        .collect();
    let mut found = None;
    for x in candidates {
        let old = w.comp::<citysim::Personality>(x).expect("p").lawfulness;
        w.comp_mut::<citysim::Personality>(x).expect("p").lawfulness = 0.2;
        if gang::recruit_gang(&w, x).is_none() {
            found = Some(x);
            break;
        }
        w.comp_mut::<citysim::Personality>(x).expect("p").lawfulness = old;
    }
    let a = found.expect("a lawless non-recruit");
    assert!(!classes::evicted_desperate(&w, a));
    let now = w.tick;
    w.comp_mut::<Household>(a).expect("h").evicted_by = Some((None, now - TICKS_PER_DAY));
    assert!(classes::evicted_desperate(&w, a));
    assert!(gang::recruit_gang(&w, a).is_some(), "a fresh lawless evictee is desperate");
    // A lawful evictee is not; nor is one evicted long ago.
    w.comp_mut::<citysim::Personality>(a).expect("p").lawfulness = 0.9;
    assert!(!classes::evicted_desperate(&w, a));
    w.comp_mut::<citysim::Personality>(a).expect("p").lawfulness = 0.2;
    w.tick = now + 30 * TICKS_PER_DAY;
    assert!(!classes::evicted_desperate(&w, a));
}
