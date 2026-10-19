//! Lives at work phase 6 (docs/LIVES_AT_WORK.md § 4, § 9.1): ages, careers
//! and leaders. Unit tests in the shipped seed-42 city with the
//! preconditions set by hand; each failed on the base `087c01e` (no
//! `systems::careers`). The probes (the laid-off loader's rehire, the
//! 64-year-old's retirement) are in `tests/person.rs`.
mod common;

use citysim::components::{Gang, GangMember, Identity, Job, Order, Profession, Retired, Role, Sentence};
use citysim::systems::{careers, demography, gang, industry, leisure, raid};
use citysim::time::DAYS_PER_YEAR;
use citysim::{Config, EntityId, EventKind, World, TICKS_PER_DAY};

fn set_age(w: &mut World, id: EntityId, years: u32) {
    if let Some(i) = w.comp_mut::<Identity>(id) {
        i.age_days = years * DAYS_PER_YEAR as u32 + 1;
    }
}

fn years(w: &World, id: EntityId) -> u32 {
    w.comp::<Identity>(id).map_or(0, |i| i.age_years())
}

fn loader(w: &World) -> Role {
    w.config.trade_role("loader").expect("the loader row")
}

/// § 4.1: a candidate outside the row's band is refused, the same one inside
/// admitted; the hiring pick never returns an out-of-band candidate; a
/// legacy role's band is its `[roles]` table's.
#[test]
fn test_band_refuses_a_hire_outside_the_row() {
    let mut w = common::world();
    assert!(careers::on(&w), "the shipped config has careers on");
    let role = loader(&w);
    assert_eq!(careers::band_cfg(&w.config, role), (18, 60));
    assert_eq!(careers::band_cfg(&w.config, Role::Guard), (18, 60));
    assert_eq!(careers::band_cfg(&w.config, Role::Ripperdoc), (25, 0));
    let id = common::jobless_housed(&w)[0];
    set_age(&mut w, id, 65);
    assert!(!careers::age_ok(&w, id, role), "a 65-year-old is refused a loader's post");
    assert!(careers::age_ok(&w, id, Role::Clerk), "an unbanded role admits him");
    set_age(&mut w, id, 30);
    assert!(careers::age_ok(&w, id, role), "the same candidate at 30 is admitted");
    w.insert(id, Retired { since: 0 });
    assert!(!careers::age_ok(&w, id, role), "the retired are out of the pool");
    // The pick: every jobless adult over 60, and the loader vacancy's pick is in band.
    let yard = w.buildings_of_kind(citysim::components::BuildingKind::FreightYard)[0];
    for a in common::adults(&w) {
        if a != id && !w.has::<Job>(a) {
            set_age(&mut w, a, 61);
        }
    }
    w.remove::<Retired>(id);
    let pick = demography::hire_candidate(&w, yard, role);
    assert_eq!(pick, Some(id), "the one in-band candidate is the pick");
    set_age(&mut w, id, 70);
    assert_eq!(demography::hire_candidate(&w, yard, role), None, "nobody in band: no hire");
    // Off, the same 70-year-old is admitted.
    let mut off = w.config.clone();
    off.strip_careers();
    w.config = off;
    assert!(careers::age_ok(&w, id, role));
}

/// § 4.2: the seeded Jobs carry seniority (capped), a hire after day 0 none;
/// the foreman is the most experienced worker of 30 or more: an older
/// short-tenure worker does not outrank a long-tenure one, and a long-tenure
/// one under 30 is passed over.
#[test]
fn test_seniority_is_seeded_and_experience_orders_the_foreman() {
    let mut w = common::world();
    let cap = u16::try_from(w.config.careers.seniority_cap_years as u64 * DAYS_PER_YEAR).unwrap();
    let seeded: Vec<u16> = w.citizens().iter().filter_map(|&a| w.comp::<Job>(a).map(|j| j.seniority_days)).collect();
    assert!(seeded.iter().filter(|&&d| d > 0).count() > seeded.len() / 2, "most seeded jobs have seniority");
    assert!(seeded.iter().all(|&d| d <= cap), "capped at {cap}");
    // A hire after day 0 has none.
    w.run_ticks(TICKS_PER_DAY);
    let hired = w.events.iter().rev().find(|e| e.kind == EventKind::Hire && e.tick > 0).map(|e| e.actors[0]);
    if let Some(h) = hired.filter(|&h| w.has::<Job>(h)) {
        assert_eq!(w.comp::<Job>(h).unwrap().seniority_days, 0, "a day-1 hire has no seniority");
    }
    // A building with three staff: A 25 years old, most tenure; B 50, little;
    // C 40, more than B. The foreman is C.
    let b = w
        .citizens()
        .iter()
        .filter_map(|&a| w.comp::<Job>(a).and_then(|j| j.employer))
        .find(|&e| w.staff_of(e).len() >= 3)
        .expect("a crew of three");
    let staff: Vec<EntityId> = w.staff_of(b).to_vec();
    for &s in &staff[3..] {
        set_age(&mut w, s, 20);
    }
    let (a, bb, c) = (staff[0], staff[1], staff[2]);
    let setup = [(a, 25, 3000u16), (bb, 50, 100), (c, 40, 500)];
    for (id, y, sen) in setup {
        set_age(&mut w, id, y);
        let j = w.comp_mut::<Job>(id).unwrap();
        j.seniority_days = sen;
        j.hired_tick = 0;
    }
    assert!(careers::experience(&w, a) > careers::experience(&w, c));
    assert_eq!(careers::foreman(&w, b, None), Some(c), "the most experienced of 30 or more");
    assert_eq!(industry::foreman(&w, b, None), Some(c), "industry's foreman reads it");
    assert_eq!(careers::foreman(&w, b, Some(c)), Some(bb), "C excluded: B, the only other over 30");
}

/// § 4.3: a profession survives a layoff and ranks its holder first for
/// the trade's vacancy (the control, the same candidate without it, is not
/// the pick); the knowledge floor admits a holder.
#[test]
fn test_profession_survives_a_layoff_and_ranks_the_rehire() {
    let mut w = common::world();
    let role = loader(&w);
    let yard = w.buildings_of_kind(citysim::components::BuildingKind::FreightYard)[0];
    // A worker laid off: the profession stays.
    let worker = w.staff_of(yard).iter().copied().find(|&s| w.has::<Job>(s)).expect("a loader");
    w.insert(worker, Profession { trade: role, days: 40, last_day: 0, next: None });
    citysim::systems::economy::dismiss(&mut w, worker, Some(yard), "laid off".into());
    assert!(!w.has::<Job>(worker));
    assert!(careers::holds(&w, worker, role), "the profession survives the layoff");
    // The laid-off loader is the pick (near, the rehire bonus): out of the
    // pool for the control (a post elsewhere), so a stranger is ranked.
    assert_eq!(demography::hire_candidate(&w, yard, role), Some(worker), "the laid-off loader first");
    demography::hire(&mut w, worker, yard, Role::Clerk);
    // The control: a candidate who is not the pick, without the profession.
    let without = demography::hire_candidate(&w, yard, role);
    let mut other = None;
    for a in common::jobless_housed(&w) {
        if a == worker || Some(a) == without || !careers::age_ok(&w, a, role) {
            continue;
        }
        w.insert(a, Profession { trade: role, days: 40, last_day: 0, next: None });
        let picked = demography::hire_candidate(&w, yard, role) == Some(a);
        w.remove::<Profession>(a);
        if picked {
            other = Some(a);
            break;
        }
    }
    let other = other.expect("a candidate the profession moves to the front");
    w.insert(other, Profession { trade: role, days: 40, last_day: 0, next: None });
    assert_eq!(demography::hire_candidate(&w, yard, role), Some(other), "the holder is picked");
    // Too few days: not yet the profession.
    w.insert(other, Profession { trade: role, days: 3, last_day: 0, next: None });
    assert_ne!(demography::hire_candidate(&w, yard, role), Some(other), "3 days is no profession");
    // A new trade replaces the profession at `profession_min_days` shifts.
    let mut p = Profession { trade: role, days: 40, last_day: 0, next: None };
    w.insert(other, p.clone());
    demography::hire(&mut w, other, yard, Role::Clerk);
    for d in 1..=w.config.careers.profession_min_days {
        w.tick = u64::from(d) * TICKS_PER_DAY;
        careers::shift_end(&mut w, other);
    }
    p = w.comp::<Profession>(other).cloned().unwrap();
    assert_eq!(p.trade, Role::Clerk, "the new trade replaced the old at the threshold: {p:?}");
}

/// A gang of at least five members with a free leader of loyalty past the
/// freelance floor, under Expand.
fn gang_of_five(w: &mut World) -> (EntityId, EntityId, Vec<EntityId>) {
    let gid = w.gang_list()[0];
    // Seven free adults enlisted (the seeded gangs start empty), veterans.
    let recruits: Vec<EntityId> =
        common::free_adults(w).into_iter().filter(|&a| w.gang_of(a).is_none() && !w.has::<Job>(a)).take(7).collect();
    for &a in &recruits {
        gang::enlist(w, a, gid);
        if let Some(m) = w.comp_mut::<GangMember>(a) {
            m.joined_tick = 0;
        }
    }
    w.tick = 30 * TICKS_PER_DAY;
    let members = w.comp::<Gang>(gid).unwrap().members.clone();
    assert!(members.len() >= 5, "a gang of five: {}", members.len());
    for &m in &members {
        w.remove::<Sentence>(m);
        if let Some(p) = w.comp_mut::<citysim::Personality>(m) {
            p.loyalty = 0.9;
        }
    }
    gang::recompute_leader(w, gid);
    let g = w.comp_mut::<Gang>(gid).unwrap();
    g.order = Order::Expand;
    let leader = g.leader.expect("a leader");
    (gid, leader, members)
}

/// § 4.5: with five fit members the leader's `following_order` is `None`
/// under Expand and the raid's march is not his; with four it is `Some`.
/// A member's is `Some` throughout. The lieutenants are ranked 1.
#[test]
fn test_leader_delegates_with_five_fit_members() {
    let mut w = common::world();
    let (gid, leader, members) = gang_of_five(&mut w);
    let fit = gang::fit_headcount(&w, gid);
    assert!(fit >= 5);
    let lts = members.iter().filter(|&&m| w.comp::<GangMember>(m).is_some_and(|g| g.rank == 1)).count();
    assert_eq!(lts, careers::lieutenant_count(&w, members.len()).min(members.len() - 1), "the lieutenants ranked 1");
    assert_eq!(gang::following_order(&w, leader), None, "the leader delegates Expand");
    let member = *members.iter().find(|&&m| m != leader).unwrap();
    assert_eq!(gang::following_order(&w, member), Some(Order::Expand));
    // A raid order: the leader does not march.
    {
        let now = w.tick;
        let g = w.comp_mut::<Gang>(gid).unwrap();
        g.order = Order::Raid;
        g.raid_at = Some(now);
    }
    assert!(!raid::raid_pending(&w, leader), "the leader stays home");
    assert!(raid::raid_pending(&w, member), "a member marches");
    // Four fit: the leader leads again.
    w.comp_mut::<Gang>(gid).unwrap().order = Order::Expand;
    let others: Vec<EntityId> = members.iter().copied().filter(|&m| m != leader).collect();
    for &m in others.iter().take(fit - 4) {
        w.insert(m, Sentence { until_tick: w.tick + 10 * TICKS_PER_DAY, crime: citysim::components::Crime::Theft });
    }
    assert_eq!(gang::fit_headcount(&w, gid), 4);
    assert_eq!(gang::following_order(&w, leader), Some(Order::Expand), "four fit: the leader works the order");
    // Lieutenants off: no delegation, no rank 1.
    let mut off = common::world_with(|c| c.gangs.strip_lieutenants());
    let (gid2, leader2, members2) = gang_of_five(&mut off);
    assert!(gang::fit_headcount(&off, gid2) >= 5);
    assert_eq!(gang::following_order(&off, leader2), Some(Order::Expand));
    assert!(!members2.iter().any(|&m| off.comp::<GangMember>(m).is_some_and(|g| g.rank == 1)));
}

/// § 4.5: the payout's cut comes off the top: the leader 20 % of the pool,
/// the lieutenants 10 % shared, then the equal shares.
#[test]
fn test_payout_cut_comes_first() {
    let mut w = common::world();
    let (gid, leader, members) = gang_of_five(&mut w);
    let lts: Vec<EntityId> =
        members.iter().copied().filter(|&m| w.comp::<GangMember>(m).is_some_and(|g| g.rank == 1)).collect();
    assert!(!lts.is_empty());
    {
        let g = w.comp_mut::<Gang>(gid).unwrap();
        g.tribute_week = 10_000;
        g.treasury = 1_000_000;
    }
    let before: Vec<i64> = members.iter().map(|&m| common::coins(&w, m)).collect();
    leisure::payout(&mut w, gid, Some(leader));
    let gain = |m: EntityId| {
        let i = members.iter().position(|&x| x == m).unwrap();
        common::coins(&w, m) - before[i]
    };
    let pool = (w.config.leisure.tribute_share * 10_000.0).floor() as i64;
    let lcut = (w.config.gangs.leader_cut * pool as f32).floor() as i64;
    let ltcut = (w.config.gangs.lieutenant_cut * pool as f32).floor() as i64;
    let share = (pool - lcut - ltcut) / members.len() as i64;
    let plain = *members.iter().find(|&&m| m != leader && !lts.contains(&m)).expect("a plain member");
    assert!((gain(plain) - share).abs() <= 1, "a member's equal share {} vs {share}", gain(plain));
    assert!((gain(leader) - (share + lcut)).abs() <= 1, "the leader's cut first: {} vs {}", gain(leader), share + lcut);
    let lt_share = ltcut / lts.len() as i64;
    assert!((gain(lts[0]) - (share + lt_share)).abs() <= 1, "a lieutenant's {} vs {}", gain(lts[0]), share + lt_share);
    let j = w.stats.current.jobs2.as_ref().expect("row");
    assert_eq!(
        (j.leader_cut_coins, j.lieutenant_cut_coins),
        (lcut, lt_share * lts.len() as i64 + ltcut % lts.len() as i64)
    );
    let e = w.events.iter().rev().find(|e| e.kind == EventKind::Collected).expect("the Collected event");
    assert!(e.text.contains("off the top"), "the split printed: {}", e.text);
}

/// The off cities (careers off, lieutenants off, both): three days write
/// nothing of the phase, and the config holds none of its keys.
#[test]
fn test_off_cities_write_nothing() {
    let edits: [fn(&mut Config); 3] = [
        |c| c.strip_careers(),
        |c| c.gangs.strip_lieutenants(),
        |c| {
            c.strip_careers();
            c.gangs.strip_lieutenants();
        },
    ];
    // The three cities side by side (the change tier's budget, 2026-10-19).
    let case = |k: usize, edit: fn(&mut Config)| {
        let mut w = common::world_with(edit);
        let (car, lts) = (k != 1, k != 0);
        w.run_ticks(3 * TICKS_PER_DAY);
        for r in &w.stats.history {
            let j = r.jobs2.as_ref().expect("jobs2 on");
            if car {
                assert_eq!((j.band_refusals, j.foremen_premium, j.profession_rehires, j.foreman_hires), (0, 0, 0, 0));
                assert_eq!((j.retirements, j.frail_exits, j.unretired), (0, 0, 0));
            }
            if lts {
                assert_eq!((j.lieutenants, j.delegated_orders, j.lieutenant_raids), (0, 0, 0));
                assert_eq!((j.leader_cut_coins, j.lieutenant_cut_coins), (0, 0));
            }
        }
        for id in w.citizens() {
            if car {
                assert!(!w.has::<Profession>(id) && !w.has::<Retired>(id));
                assert!(w.comp::<Job>(id).is_none_or(|j| j.seniority_days == 0 && !j.foreman));
            }
            if lts {
                assert!(w.comp::<GangMember>(id).is_none_or(|g| g.rank != 1 && g.raids_led == 0));
            }
        }
        if car {
            assert!(!w.events.iter().any(|e| e.kind == EventKind::Retired));
        }
        let text = ron::to_string(&w.config).expect("config");
        if car {
            assert!(!text.contains("careers:") && !text.contains("roles:") && !text.contains("min_age:"), "case {k}");
        }
        if lts {
            assert!(!text.contains("lieutenant") && !text.contains("leader_cut"), "case {k}");
        }
        let save = citysim::save::to_ron(&w);
        if car {
            assert!(!save.contains("seniority_days") && !save.contains("profession:"), "case {k}");
        }
        if lts {
            assert!(!save.contains("raids_led"), "case {k}");
        }
    };
    std::thread::scope(|s| {
        let hs: Vec<_> = edits.into_iter().enumerate().map(|(k, edit)| s.spawn(move || case(k, edit))).collect();
        for h in hs {
            h.join().expect("an off city");
        }
    });
    // The on config holds them.
    let on = Config::load();
    assert!(on.careers.enabled && on.gangs.lieutenants && !on.roles.is_empty());
}

/// The diag device (§ 9.1 Phase 6): the physical trades' ages on day 19
/// and each staffed building's foreman (age, experience rank), careers on
/// against off, seeds 42-44.
#[test]
#[ignore = "tier: diag"]
fn diag_physical_trades_ages() {
    let physical = ["loader", "builder", "packer", "day_labourer", "bouncer"];
    for seed in [42u64, 43, 44] {
        for on in [false, true] {
            let mut cfg = Config::load();
            if !on {
                cfg.strip_careers();
            }
            let mut w = World::new(seed, cfg);
            w.run_ticks(19 * TICKS_PER_DAY);
            let mut ages: Vec<u32> = Vec::new();
            for a in w.citizens() {
                let Some(j) = w.comp::<Job>(a) else { continue };
                let phys = matches!(j.role, Role::Guard | Role::Fighter)
                    || physical.iter().any(|k| w.config.trade_role(k) == Some(j.role));
                if phys {
                    ages.push(years(&w, a));
                }
            }
            let over60 = ages.iter().filter(|&&y| y > 60).count();
            let over70 = ages.iter().filter(|&&y| y >= 70).count();
            let mut hist = [0usize; 6];
            for &y in &ages {
                hist[((y.saturating_sub(18)) / 10).min(5) as usize] += 1;
            }
            // Foremen: every building with two or more staff.
            let mut employers: Vec<EntityId> =
                w.citizens().iter().filter_map(|&a| w.comp::<Job>(a).and_then(|j| j.employer)).collect();
            employers.sort();
            employers.dedup();
            let (mut n, mut under30, mut not_top, mut age_sum) = (0, 0, 0, 0u32);
            for b in employers.into_iter().filter(|&b| w.staff_of(b).len() >= 2) {
                let Some(f) = industry::foreman(&w, b, None) else { continue };
                n += 1;
                age_sum += years(&w, f);
                under30 += usize::from(years(&w, f) < 30);
                let best = w.staff_of(b).iter().map(|&s| careers::experience(&w, s)).max().unwrap_or(0);
                not_top += usize::from(careers::experience(&w, f) < best);
            }
            println!(
                "PROBE careers seed {seed} {}: physical {} workers, over 60 {over60}, 70+ {over70}, by decade 18-27.. {hist:?}; \
                 foremen {n}, mean age {:.1}, under 30 {under30}, not the most experienced {not_top}",
                if on { "on " } else { "off" },
                ages.len(),
                f64::from(age_sum) / f64::from(n.max(1) as u32),
            );
        }
    }
}
