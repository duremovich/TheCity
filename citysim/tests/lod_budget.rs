//! L2 phase 3 (docs/LIFE_L2.md § 4, plan L21-L23, L29-L30): held
//! prisoners, the gang quota and its rotation, the watch off shift, the
//! Statistical GangWork day, the scavenge dry hour, bed reservations.

use citysim::exec::actions;
use citysim::exec::ReservationKind;
use citysim::ledger::{ActKind, VictimClass, ViolenceSource};
use citysim::systems::{fviolence, gang, law, lod, street};
use citysim::{
    ActionKind, Brain, Building, BuildingKind, Config, Crime, EntityId, EventKind, Gang, GoalKind, Job, Lod, Needs,
    Order, Personality, Position, StepResult, Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

/// The v1 city (the LOD budget is always on since 2026-10-09).
fn budget_cfg() -> Config {
    Config::load().v1_profile()
}

fn world(seed: u64) -> World {
    World::new(seed, budget_cfg())
}

/// Jobless adults with a Brain, free, not in a gang.
fn civilians(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&id| !w.has::<Job>(id) && w.has::<Brain>(id) && w.gang_of(id).is_none())
        .filter(|&id| citysim::systems::demography::is_adult(w, id))
        .take(n)
        .collect()
}

fn jail_for(w: &mut World, who: EntityId, ticks: u64) -> EntityId {
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let until = w.tick + ticks;
    law::sentence(w, who, Crime::Assault, until, jail);
    jail
}

fn lod_of(w: &World, id: EntityId) -> Lod {
    w.comp::<Brain>(id).expect("brain").lod
}

fn assign_at(w: &mut World, tick: u64) {
    w.tick = tick;
    lod::run(w);
}

#[test]
fn test_held_prisoner_fed_daily_and_never_leaves_jail() {
    let mut w = world(301);
    w.tick = 120;
    let p = civilians(&w, 1)[0];
    let jail = jail_for(&mut w, p, 10 * TICKS_PER_DAY);
    w.run_ticks(TICKS_PER_HOUR); // the next hourly assignment
    assert_eq!(lod_of(&w, p), Lod::Statistical, "held in the cells");
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.has::<citysim::Sentence>(p));
    assert_eq!(lod_of(&w, p), Lod::Statistical);
    assert_eq!(w.comp::<Position>(p).and_then(|q| q.building), Some(jail), "never snapped out of the Jail");
    let n = w.comp::<Needs>(p).expect("needs");
    assert!(n.hunger > 0.2 && n.starving_since.is_none(), "fed: hunger {}", n.hunger);
    // Three midnights in the window; the last one's meal is in today's open row.
    let fed: u32 = w.stats.history.iter().map(|r| r.budget.held_fed).sum::<u32>() + w.stats.current.budget.held_fed;
    assert!(fed >= 3, "jail_upkeep fed the held prisoner each midnight ({fed})");
    assert!(w.stats.history.iter().all(|r| r.budget.tier_held >= 1));
}

#[test]
fn test_held_prisoner_promoted_before_release() {
    let mut w = world(302);
    let start = 10 * TICKS_PER_HOUR;
    w.tick = start;
    let p = civilians(&w, 1)[0];
    let jail = jail_for(&mut w, p, 5 * TICKS_PER_HOUR);
    assign_at(&mut w, start + TICKS_PER_HOUR);
    assert_eq!(lod_of(&w, p), Lod::Statistical, "4 h left: held");
    // Two hours before release: a body, still inside.
    assign_at(&mut w, start + 3 * TICKS_PER_HOUR);
    assert_eq!(lod_of(&w, p), Lod::Coarse, "release soon: Coarse");
    assert_eq!(w.comp::<Position>(p).and_then(|q| q.building), Some(jail), "promoted inside the Jail");
    w.tick = start + 3 * TICKS_PER_HOUR + 1;
    w.run_ticks(2 * TICKS_PER_HOUR + 5);
    assert!(!w.has::<citysim::Sentence>(p), "released");
    let door = w.comp::<Building>(jail).map(|b| w.outside_door(b)).expect("door");
    let pos = w.comp::<Position>(p).expect("pos");
    assert!(pos.building != Some(jail));
    assert!(pos.tile.manhattan(door) <= 40, "walked out from the Jail door");
}

#[test]
fn test_cuffed_and_breakout_prisoners_are_coarse() {
    let mut w = world(303);
    w.tick = 60;
    let gangs = w.gangs();
    let g0 = gangs[0];
    let civ = civilians(&w, 3);
    let (plain, cuffed, member) = (civ[0], civ[1], civ[2]);
    gang::enlist(&mut w, member, g0);
    for &p in &civ {
        jail_for(&mut w, p, 10 * TICKS_PER_DAY);
    }
    let guard = w.guards()[0];
    w.comp_mut::<Brain>(cuffed).expect("b").cuffed_by = Some(guard);
    // The gang's BreakOut musters in ten hours: outside the window.
    {
        let g = w.comp_mut::<Gang>(g0).expect("gang");
        g.order = Order::BreakOut;
        g.raid_at = Some(120 + 10 * TICKS_PER_HOUR);
    }
    assign_at(&mut w, 120);
    assert_eq!(lod_of(&w, plain), Lod::Statistical);
    assert_eq!(lod_of(&w, cuffed), Lod::Coarse, "cuffed: a body");
    assert_eq!(lod_of(&w, member), Lod::Statistical, "the breakout is ten hours off");
    w.comp_mut::<Gang>(g0).expect("gang").raid_at = Some(180 + 2 * TICKS_PER_HOUR);
    assign_at(&mut w, 180);
    assert_eq!(lod_of(&w, member), Lod::Coarse, "in the raid window the gang's prisoner holds a body");
    assert_eq!(lod_of(&w, plain), Lod::Statistical);
    // A breakout's escape of a held prisoner promotes it first.
    law::escape(&mut w, plain);
    assert_eq!(lod_of(&w, plain), Lod::Coarse);
}

#[test]
fn test_raid_window_opens_quota() {
    let mut w = world(304);
    w.config.gangs.max_members = 200;
    let g0 = w.gangs()[0];
    for m in civilians(&w, 60) {
        gang::enlist(&mut w, m, g0);
    }
    w.tick = 600;
    assert_eq!(lod::gang_quota_members(&w, g0).len(), w.config.lod.gang_quota);
    {
        let g = w.comp_mut::<Gang>(g0).expect("gang");
        g.order = Order::Raid;
        g.raid_at = Some(600 + 3 * TICKS_PER_HOUR);
    }
    assert!(lod::raid_window(&w, g0));
    assert_eq!(lod::gang_quota_members(&w, g0).len(), w.config.lod.raid_quota, "the muster opens the quota");
    w.comp_mut::<Gang>(g0).expect("gang").raid_at = None;
    assert_eq!(lod::gang_quota_members(&w, g0).len(), w.config.lod.gang_quota, "the raid over, the quota closes");
}

#[test]
fn test_off_shift_guard_ranks_as_civilian() {
    let mut w = world(305);
    w.config.lod.watch_on_shift_only = true;
    let guard = w
        .guards()
        .iter()
        .copied()
        .find(|&g| !law::is_private_guard(&w, g) && w.comp::<Job>(g).is_some_and(|j| !j.shifts.is_empty()))
        .expect("a city guard");
    let (s, _) = w.comp::<Job>(guard).expect("job").shifts[0];
    let job = w.comp::<Job>(guard).expect("job").clone();
    w.tick = u64::from(s) + 5;
    assert!(job.on_shift(w.tick_of_day()));
    assert_eq!(lod::rank_class(&w, guard), 3, "on shift: the watch");
    // The tick of the day farthest from any shift (and its hour of lead).
    let off = (0..1440u16).find(|&t| !job.on_shift(t) && !job.on_shift((t + 60) % 1440)).expect("an off-shift hour");
    w.tick = TICKS_PER_DAY + u64::from(off);
    assert_eq!(lod::rank_class(&w, guard), 0, "off shift: a civilian");
    w.config.lod.watch_on_shift_only = false;
    assert_eq!(lod::rank_class(&w, guard), 3, "the rule off (the shipped config): the M10 watch");
}

#[test]
fn test_stat_member_extorts_territory_home() {
    let mut w = world(306);
    w.config.gangs.stat_extort_prior = 1.0;
    w.config.gangs.stat_prior_days = 1000.0;
    w.config.crime.stat_theft_caught_p = 0.0;
    if let Some(t) = w.stat_table.as_mut() {
        t.p_theft_caught = Some(0.0);
    }
    let g0 = w.gangs()[0];
    let m = civilians(&w, 1)[0];
    gang::enlist(&mut w, m, g0);
    w.comp_mut::<Personality>(m).expect("p").loyalty = 0.95;
    w.comp_mut::<Gang>(g0).expect("gang").order = Order::Expand;
    w.tick = 10 * TICKS_PER_HOUR; // the Work phase
    lod::set_lod(&mut w, m, Lod::Statistical);
    assert_eq!(gang::following_order(&w, m), Some(Order::Expand));
    let home = gang::stat_extort_target(&w, m, Order::Expand).expect("an unclaimed inhabited Home");
    for r in w.residents_of(home).to_vec() {
        if let Some(wl) = w.comp_mut::<Wallet>(r) {
            wl.coins = 50;
        }
    }
    let before = w.comp::<Gang>(g0).expect("gang").treasury;
    let witnesses = w.events.iter().filter(|e| e.kind == EventKind::Witness).count();
    fviolence::stat_gang_day(&mut w, m);
    let after = w.comp::<Gang>(g0).expect("gang").treasury;
    assert!(after > before, "the gang treasury rises ({before} -> {after})");
    assert_eq!(w.comp::<Brain>(m).expect("b").gang_task_day, Some(w.day()));
    assert_eq!(w.stats.current.budget.stat_extorts, 1);
    assert_eq!(w.events.iter().filter(|e| e.kind == EventKind::Witness).count(), witnesses, "no witness");
    assert!(w.comp::<Building>(home).and_then(|b| b.claim).is_some_and(|c| c.gang == g0), "the claim advances");
    // Once a day.
    fviolence::stat_gang_day(&mut w, m);
    assert_eq!(w.stats.current.budget.stat_extorts, 1);
}

#[test]
fn test_ledger_counts_body_acts_and_member_days() {
    let mut w = world(307);
    let g0 = w.gangs()[0];
    let m = civilians(&w, 1)[0];
    gang::enlist(&mut w, m, g0);
    w.comp_mut::<Personality>(m).expect("p").loyalty = 0.95;
    w.comp_mut::<Gang>(g0).expect("gang").order = Order::Expand;
    lod::set_lod(&mut w, m, Lod::Coarse);
    w.tick = 2 * TICKS_PER_DAY + 600;
    fviolence::note_act(&mut w, m, ActKind::Extort);
    let d = fviolence::member_district(&w, m).expect("district");
    let key = (ViolenceSource::Order(Order::Expand), d, VictimClass::Member);
    assert_eq!(w.order_rates.cells[&key].acts_today, [1, 0, 0]);
    w.comp_mut::<Brain>(m).expect("b").body_day = Some(2);
    w.tick = 3 * TICKS_PER_DAY;
    fviolence::daily_actor(&mut w);
    let cell = &w.order_rates.cells[&key];
    assert_eq!(cell.acts_sum(ActKind::Extort), 1);
    assert!(cell.member_days_sum() >= 1);
    let p = fviolence::act_rate(&w, Order::Expand, d, ActKind::Extort);
    let days = w.config.gangs.stat_prior_days;
    let expect = (1.0 + w.config.gangs.stat_extort_prior * days) / (cell.member_days_sum() as f32 + days);
    assert!((p - expect).abs() < 1e-6);
    // A Statistical member's act is not the ledger's.
    lod::set_lod(&mut w, m, Lod::Statistical);
    fviolence::note_act(&mut w, m, ActKind::Extort);
    assert_eq!(w.order_rates.cells[&key].acts_today, [0, 0, 0]);
    // The ledger round-trips a save (tuple keys in RON).
    let text = citysim::save::to_ron(&w);
    let back = citysim::save::from_ron(&text).expect("load");
    assert_eq!(back.order_rates, w.order_rates);
}

#[test]
fn test_dry_scavenge_is_done_and_cools_earn() {
    let mut w = world(308);
    w.config.life = Config::load().life;
    w.config.life.scavenge_p = 0.0;
    let a = civilians(&w, 1)[0];
    w.leave_building(a);
    let max = w.config.life.scavenge_dry_max;
    for i in 1..=max {
        let r = {
            let t = w.tick;
            actions::on_complete(&mut w, a, ActionKind::Scavenge, None, t, t)
        };
        assert_eq!(r, StepResult::Done, "a dry hour is an hour spent");
        let dry = w.comp::<Brain>(a).expect("b").scavenge_dry;
        if i < max {
            assert_eq!(dry, i);
        }
    }
    let b = w.comp::<Brain>(a).expect("b");
    assert_eq!(b.scavenge_dry, 0, "the streak resets at the cooldown");
    let cool = w.tick + w.config.life.scavenge_cool_hours * TICKS_PER_HOUR;
    assert_eq!(b.cooldowns.get(&GoalKind::Earn).copied(), Some(cool), "Earn cools");
}

#[test]
fn test_reserved_bed_cannot_be_taken() {
    // The 2,000 city: Hotels, the life pass and the budget on.
    let mut w = World::new(42, Config::load());
    let hotels: Vec<EntityId> =
        w.buildings_of_kind(BuildingKind::Hotel).iter().copied().filter(|&h| street::is_hotel(&w, h)).collect();
    let h = hotels[0];
    // One bed left there.
    let beds = usize::from(w.comp::<Building>(h).expect("hotel").capacity);
    let taken = beds - street::free_beds(&w, h);
    w.comp_mut::<Building>(h).expect("hotel").capacity = u8::try_from(taken + 1).expect("u8");
    assert_eq!(street::free_beds(&w, h), 1);
    let door = w.comp::<Building>(h).map(|b| w.outside_door(b)).expect("door");
    let pick: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&id| w.has::<Brain>(id) && citysim::systems::demography::is_adult(&w, id) && !w.has::<Job>(id))
        .take(2)
        .collect();
    for &a in &pick {
        w.set_home(a, None);
        w.leave_building(a);
        w.comp_mut::<Position>(a).expect("pos").tile = door;
        w.comp_mut::<Wallet>(a).expect("w").coins = 500;
    }
    let (a, b) = (pick[0], pick[1]);
    assert_eq!(street::hotel_for(&w, a), Some(h));
    assert_eq!(street::hotel_for(&w, b), Some(h));
    // A plans the last bed: it holds it until it arrives.
    let until = w.tick + 600;
    w.reserve(a, ReservationKind::Bed { home: h }, until);
    assert_eq!(street::hotel_for(&w, a), Some(h), "its own reservation does not shut it out");
    assert_ne!(street::hotel_for(&w, b), Some(h), "the second planner finds another Hotel or none");
    assert_eq!(street::free_beds(&w, h), 0);
    // A checks in: the reservation is the booking, the bed is A's.
    w.leave_building(a);
    w.enter_building(a, h);
    assert!(street::check_in(&mut w, a, h).is_ok());
    w.release_all(a);
    assert_eq!(street::free_beds(&w, h), 0);
    assert!(street::check_in(&mut w, b, h).is_err() || w.comp::<Position>(b).and_then(|p| p.building) != Some(h));
}

/// Spec review (L21): a held prisoner is never an off-screen stranger, a
/// housemate partner for a lonely Statistical agent, or a hack candidate.
#[test]
fn test_held_prisoner_is_no_partner_or_runner() {
    let mut w = world(309);
    // Every meeting and every chat fires, and a chat never stays at home.
    if let Some(t) = w.stat_table.as_mut() {
        for r in &mut t.rows {
            r.p_meet = 1.0;
            r.p_chat = 1.0;
            r.p_chat_home = 0.0;
        }
    }
    let jail = w.building_of_kind(BuildingKind::Jail).expect("jail");
    let jail_zone = w.comp::<Building>(jail).map(|b| w.map.zone(b.door)).expect("zone");
    // A lonely free agent whose housemates are all jailed.
    let lonely = w
        .citizens()
        .into_iter()
        .filter(|&a| w.has::<Brain>(a) && citysim::systems::demography::is_adult(&w, a) && !w.has::<Job>(a))
        .find(|&a| {
            w.comp::<citysim::Household>(a)
                .and_then(|h| h.home)
                .is_some_and(|h| w.residents_of(h).iter().any(|&r| r != a && w.has::<Brain>(r)))
        })
        .expect("a housed adult with housemates");
    let home = w.comp::<citysim::Household>(lonely).and_then(|h| h.home).expect("home");
    let mates: Vec<EntityId> =
        w.residents_of(home).iter().copied().filter(|&r| r != lonely && w.has::<Brain>(r)).collect();
    for o in w.neighbours(lonely).collect::<Vec<_>>() {
        w.remove_edge(lonely, o);
    }
    // Everyone else standing in the Jail's zone goes to the cells but five.
    let mut prisoners: Vec<EntityId> = mates.clone();
    let mut free_left = 5;
    for a in w.citizens() {
        if a == lonely || prisoners.contains(&a) || !w.has::<Brain>(a) {
            continue;
        }
        if !w.comp::<Position>(a).is_some_and(|p| w.map.zone(p.tile) == jail_zone) {
            continue;
        }
        if free_left > 0 {
            free_left -= 1;
            continue;
        }
        prisoners.push(a);
    }
    w.tick = 120;
    for &p in &prisoners {
        jail_for(&mut w, p, 10 * TICKS_PER_DAY);
    }
    assign_at(&mut w, 180);
    lod::set_lod(&mut w, lonely, Lod::Statistical);
    assert!(prisoners.iter().all(|&p| lod_of(&w, p) == Lod::Statistical), "all held");
    let edges = |w: &World| -> usize {
        w.citizens()
            .into_iter()
            .filter(|a| !prisoners.contains(a))
            .map(|a| prisoners.iter().filter(|&&p| w.edge(a, p).is_some()).count())
            .sum()
    };
    let before = edges(&w);
    for t in 181..241 {
        w.tick = t;
        lod::run_statistical(&mut w);
    }
    assert_eq!(edges(&w), before, "no free Statistical agent met or chatted with a held prisoner");
    for &m in &mates {
        assert!(w.edge(lonely, m).is_none(), "a jailed housemate is no chat partner");
    }

    // The Virt pass: a held prisoner with a deck gets no run order.
    let mut cfg = Config::load();
    cfg.hack.stat_hack_min = 0.0;
    let mut v = World::new(42, cfg);
    let a = v
        .citizens()
        .into_iter()
        .find(|&a| {
            v.has::<Brain>(a)
                && citysim::systems::demography::is_adult(&v, a)
                && v.gang_of(a).is_none()
                && !law::is_guard(&v, a)
                && v.comp::<citysim::Kit>(a).is_some_and(|k| k.deck.is_none() && !k.chrome)
        })
        .expect("an adult");
    citysim::systems::assets::grant(&mut v, a, citysim::AssetKind::Deck, 2).expect("deck");
    v.comp_mut::<citysim::Skills>(a).expect("skills").hacking = 0.6;
    v.tick = 60;
    jail_for(&mut v, a, 30 * TICKS_PER_DAY);
    assign_at(&mut v, 120);
    assert_eq!(lod_of(&v, a), Lod::Statistical, "held");
    let day = (7 - u64::from(a.index) % 7) % 7 + 7;
    v.tick = day * TICKS_PER_DAY;
    citysim::systems::virt::stat_pass(&mut v);
    assert_eq!(lod_of(&v, a), Lod::Statistical, "no promotion out of the cells");
    assert!(!v.run_orders.contains_key(&a), "no run order for a prisoner");
}

/// Spec review (L21): a prisoner held from mid-afternoon is decayed for
/// its held hours only at midnight (the body's per-tick decay did the rest).
#[test]
fn test_held_decay_counts_only_held_hours() {
    let mut w = world(310);
    let p = civilians(&w, 1)[0];
    w.tick = 15 * TICKS_PER_HOUR;
    jail_for(&mut w, p, 10 * TICKS_PER_DAY);
    assign_at(&mut w, 16 * TICKS_PER_HOUR);
    assert_eq!(lod_of(&w, p), Lod::Statistical);
    assert_eq!(w.held_since.get(&p).copied(), Some(16 * TICKS_PER_HOUR));
    w.comp_mut::<Needs>(p).expect("needs").hunger = 1.0;
    w.tick = TICKS_PER_DAY;
    law::held_daily(&mut w);
    let jailed_per_tick = w.config.needs.hunger_decay_per_tick * 0.5;
    let expect = 1.0 - jailed_per_tick * (8 * TICKS_PER_HOUR) as f32;
    let got = w.comp::<Needs>(p).expect("needs").hunger;
    assert!((got - expect).abs() < 1e-4, "8 held hours decayed: {got} vs {expect}");
    assert_eq!(w.held_since.get(&p).copied(), Some(TICKS_PER_DAY), "the clock restarts");
}
