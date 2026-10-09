//! Jobs v2 P5a (docs/JOBS_V2.md § 5; plan J16-J18): the civic budget. With
//! `[economy2] wages` and `no_safety_net` on, the Treasury's midnight pass
//! turns a share of its trailing receipts (plus a payout of the hoard above
//! the band) into the police and Sanitation headcounts, police first; a
//! headcount drops at most `civic_pace` a day; nothing is spent at or under
//! `band[0]`; a god's `SetGuardCount`/`SetSanitation` pins both until
//! `SetCivicAuto`. The budget moves no coins (the posts are paid at shift
//! end), so the coin identity holds.

use citysim::components::Role;
use citysim::systems::{econ, treasury};
use citysim::{Config, EventKind, PlayerCommand, World, TICKS_PER_DAY};

/// The wages + no-net city.
fn config() -> Config {
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    cfg.economy2.no_safety_net = true;
    cfg
}

fn city() -> World {
    let w = World::new(42, config());
    assert!(econ::no_net(&w) && treasury::on(&w), "the target city has wages on and no net");
    w
}

fn set_treasury(w: &mut World, coins: i64) {
    w.treasury_mut().expect("the Treasury").coins = coins;
}

/// A ring of `days` equal receipts.
fn set_receipts(w: &mut World, per_day: i64, days: usize) {
    w.econ.receipts = std::iter::repeat_n(per_day, days).collect();
}

fn police_by_pop(w: &World) -> i64 {
    (f64::from(w.config.treasury.police_per_1000) * w.population() as f64 / 1000.0).round() as i64
}

/// J16: the target is `civic_share` x the mean receipts plus the hoard above
/// `band[1]` over `payout_days`; police are filled first up to the ceiling,
/// then Sanitation up to `sanitation_max`, each at its wage.
#[test]
fn test_civic_allocation_police_first_then_sanitation() {
    let mut w = city();
    let cfg = w.config.treasury.clone();
    let gw = w.config.economy.wage(Role::Guard);
    let sw = treasury::sweeper_wage(&w);
    let cap = treasury::police_ceiling(&w);
    assert!(cap >= 36, "the police ceiling at least the shipped headcount: {cap}");

    // Inside the band: the receipts' share alone.
    set_treasury(&mut w, (cfg.band[0] + cfg.band[1]) / 2);
    set_receipts(&mut w, 500, 14);
    let target = treasury::civic_target(&w);
    assert_eq!(target, (f64::from(cfg.civic_share) * 500.0).round() as i64);
    // A small budget buys police only.
    let (g, s) = treasury::civic_allocation(&w, 20 * gw + sw - 1);
    assert_eq!((i64::from(g), s), (20, 0), "police first: {g} guards, {s} sweepers");
    // Past the police ceiling, the rest buys sweepers.
    let (g, s) = treasury::civic_allocation(&w, cap * gw + 7 * sw + 3);
    assert_eq!((i64::from(g), s), (cap, 7), "the ceiling, then Sanitation");
    // Sanitation stops at its ceiling: the rest is not spent.
    let (g, s) = treasury::civic_allocation(&w, cap * gw + 1_000 * sw);
    assert_eq!((i64::from(g), s), (cap, cfg.sanitation_max));

    // Above the band the hoard pays out over `payout_days`.
    set_treasury(&mut w, cfg.band[1] + 60_000);
    let hoard = 60_000 / i64::from(cfg.payout_days);
    assert_eq!(treasury::civic_target(&w), target + hoard);
}

/// J16: the budget is spent only above `band[0]`: at or under it the target
/// is 0 whatever the receipts, and the headcounts shrink.
#[test]
fn test_civic_spending_stops_under_band_lo() {
    let mut w = city();
    let lo = w.config.treasury.band[0];
    set_receipts(&mut w, 5_000, 14);
    set_treasury(&mut w, lo);
    assert_eq!(treasury::civic_target(&w), 0, "nothing spent at band[0]");
    set_treasury(&mut w, lo - 1_000);
    assert_eq!(treasury::civic_target(&w), 0, "nothing spent under band[0]");
    let (g, s) = (w.levers.guard_count, w.levers.sanitation_count);
    treasury::civic_daily(&mut w);
    assert!(w.levers.guard_count < g && w.levers.sanitation_count < s, "the headcounts shrink");
    set_treasury(&mut w, lo + 1);
    assert!(treasury::civic_target(&w) > 0, "spent again just above band[0]");
}

/// J16: a headcount never drops more than `civic_pace` a day; it rises at
/// once (the reconcilers hire five a day).
#[test]
fn test_civic_headcounts_drop_at_the_pace() {
    let mut w = city();
    let pace = w.config.treasury.civic_pace;
    assert_eq!(pace, 5);
    assert_eq!(treasury::paced(60, 0, pace), 55);
    assert_eq!(treasury::paced(3, 0, pace), 0);
    assert_eq!(treasury::paced(10, 40, pace), 40);
    // In the daily pass: from 60 guards and 30 sweepers to a target of 0.
    let lo = w.config.treasury.band[0];
    set_treasury(&mut w, lo - 1);
    w.econ.receipts.clear();
    w.levers.guard_count = 60;
    w.levers.sanitation_count = 30;
    let mut seen = vec![];
    for _ in 0..4 {
        // `civic_daily` pushes yesterday's row; a fresh world has none, so
        // keep one day of receipts in the ring.
        set_receipts(&mut w, 100, 1);
        treasury::civic_daily(&mut w);
        seen.push((w.levers.guard_count, w.levers.sanitation_count));
    }
    assert_eq!(seen, vec![(55, 25), (50, 20), (45, 15), (40, 10)]);
    // No receipts yet (the first midnight): the configured headcounts hold.
    let mut fresh = city();
    let (g, s) = (fresh.levers.guard_count, fresh.levers.sanitation_count);
    set_treasury(&mut fresh, 0);
    treasury::civic_daily(&mut fresh);
    assert_eq!((fresh.levers.guard_count, fresh.levers.sanitation_count), (g, s));
}

/// J17: `SetGuardCount` or `SetSanitation` pins both headcounts against the
/// budget; `SetCivicAuto` releases them. Without the band, neither pins and
/// the release fails.
#[test]
fn test_civic_pin_and_release() {
    let mut w = city();
    let hi = w.config.treasury.band[1];
    set_treasury(&mut w, hi + 100_000);
    set_receipts(&mut w, 2_000, 14);
    w.push_command(PlayerCommand::SetGuardCount(12));
    w.apply_commands();
    assert!(w.econ.civic_pinned, "SetGuardCount pins with no_net");
    let sanitation = w.levers.sanitation_count;
    for _ in 0..5 {
        treasury::civic_daily(&mut w);
    }
    assert_eq!(w.levers.guard_count, 12, "pinned: the budget does not move the police");
    assert_eq!(w.levers.sanitation_count, sanitation, "pinned: nor Sanitation");
    w.push_command(PlayerCommand::SetCivicAuto);
    w.apply_commands();
    assert!(!w.econ.civic_pinned, "SetCivicAuto releases the pin");
    treasury::civic_daily(&mut w);
    assert!(w.levers.guard_count > 12, "released, the budget hires police: {}", w.levers.guard_count);
    w.push_command(PlayerCommand::SetSanitation(3));
    w.apply_commands();
    assert!(w.econ.civic_pinned, "SetSanitation pins too");

    // With the net (no band): no pin, and the release is refused.
    let mut cfg = Config::load();
    cfg.economy2.wages = true;
    let mut twin = World::new(42, cfg);
    assert!(!treasury::on(&twin));
    twin.push_command(PlayerCommand::SetGuardCount(12));
    twin.push_command(PlayerCommand::SetCivicAuto);
    twin.apply_commands();
    assert!(!twin.econ.civic_pinned, "no pin without the band");
    assert!(twin.events.iter().any(|e| e.kind == EventKind::PlayerActionFailed && e.text.contains("SetCivicAuto")));
}

/// P5a (plan risk 8): the police ceiling is the smaller of the population's
/// (`police_per_1000`) and the Jail's room (`police_per_jail_bed` x capacity):
/// a bigger Jail lifts it, up to the population's.
#[test]
fn test_police_ceiling_capped_by_jail_room() {
    let mut cfg = config();
    let per_bed = cfg.treasury.police_per_jail_bed;
    let capacity = cfg.buildings.jail.capacity;
    let w = World::new(42, cfg.clone());
    let by_jail = (f64::from(per_bed) * f64::from(capacity)).round() as i64;
    assert!(by_jail < police_by_pop(&w), "the shipped Jail binds before the population");
    assert_eq!(treasury::police_ceiling(&w), by_jail);
    // The budget past the Jail's cap buys sweepers, not police.
    let gw = w.config.economy.wage(Role::Guard);
    let (g, s) = treasury::civic_allocation(&w, (by_jail + 10) * gw);
    assert_eq!(i64::from(g), by_jail);
    assert!(s > 0);
    // A bigger Jail lifts the ceiling (to the population's at most).
    cfg.buildings.jail.capacity = u8::MAX;
    let big = World::new(42, cfg);
    let by_big = (f64::from(per_bed) * f64::from(u8::MAX)).round() as i64;
    assert!(by_big > by_jail);
    assert_eq!(treasury::police_ceiling(&big), police_by_pop(&big).min(by_big));
}

/// J16, J34: over 20 days of the target city the budget moves no coins
/// (the coin identity holds every day), the receipts ring fills to
/// `receipts_days`, the Treasury pays a civic payroll, and the headcounts
/// the budget set are in the day's row.
#[test]
fn test_civic_budget_conserves_coins_over_a_run() {
    let mut w = city();
    let start = econ::identity(&w);
    for day in 0..20 {
        w.run_ticks(TICKS_PER_DAY);
        assert_eq!(econ::identity(&w), start, "the coin identity moved on day {day}");
    }
    assert_eq!(w.econ.receipts.len(), w.config.treasury.receipts_days as usize);
    let rows: Vec<_> = w.stats.history.iter().collect();
    assert!(rows.iter().all(|r| r.jobs.civic_payroll >= 0));
    assert!(rows.iter().map(|r| r.jobs.civic_payroll).sum::<i64>() > 0, "the Treasury paid its posts");
    let last = rows.last().expect("a closed day");
    assert_eq!(last.jobs.guard_count, w.levers.guard_count);
    assert_eq!(last.jobs.sanitation_count, w.levers.sanitation_count);
    // The ring's newest entry is the four receipts of the day before the
    // last midnight (the last row closed at 23:59 after it).
    let prev = rows[rows.len() - 2];
    let r = prev.flow_tax + prev.jobs.flow_fines + prev.econ.flow_customs + prev.econ.flow_property;
    assert_eq!(*w.econ.receipts.back().expect("receipts"), r);
}

/// J16, J17: `EconState.receipts` and `civic_pinned` survive a save round
/// trip, and the headcounts are unchanged across the load.
#[test]
fn test_civic_state_survives_a_save() {
    let mut w = city();
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(!w.econ.receipts.is_empty(), "receipts after three midnights");
    w.push_command(PlayerCommand::SetGuardCount(22));
    w.apply_commands();
    assert!(w.econ.civic_pinned);
    let back = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("the save loads");
    assert_eq!(back.econ.receipts, w.econ.receipts);
    assert!(back.econ.civic_pinned);
    assert_eq!(back.levers.guard_count, 22);
    assert_eq!(back.levers.sanitation_count, w.levers.sanitation_count);
    assert_eq!(back.econ, w.econ);
}

/// Flip readiness: `Config::scaled_to` scales the no-net Treasury's coin
/// keys (the band, the kept balance, the till float, the Sanitation
/// ceiling), so a 300-resident no-net city keeps its police: unscaled, its
/// Treasury sat under the 2,000 city's band, the civic target read 0 and
/// the first midnight dismissed every guard (the district beats were empty
/// a day later, with or without a save in between).
#[test]
fn test_scaled_no_net_city_keeps_its_police_and_beats() {
    let full = config();
    let cfg = config().scaled_to(300);
    let f = 300.0 / f64::from(full.world.population);
    let s = |v: i64| (v as f64 * f).round() as i64;
    assert_eq!(cfg.treasury.band, [s(full.treasury.band[0]), s(full.treasury.band[1])]);
    assert_eq!(cfg.treasury.treasury_initial, s(full.treasury.treasury_initial));
    assert_eq!(cfg.treasury.till_initial, s(full.treasury.till_initial));
    let mut w = World::new(19, cfg);
    assert!(econ::no_net(&w));
    w.run_ticks(TICKS_PER_DAY + 10);
    assert!(w.levers.guard_count > 0, "the first midnight keeps police");
    assert!(!w.law().expect("law").beats.is_empty(), "the first midnight deals the beats");
    let mut back = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("the save loads");
    back.run_ticks(TICKS_PER_DAY);
    w.run_ticks(TICKS_PER_DAY);
    for x in [&w, &back] {
        assert!(x.levers.guard_count > 0, "police a day later: lever {}", x.levers.guard_count);
        assert!(!x.law().expect("law").beats.is_empty(), "beats a day later");
    }
    assert_eq!(back.law().map(|l| l.beats.clone()), w.law().map(|l| l.beats.clone()), "the load runs on alike");
}
