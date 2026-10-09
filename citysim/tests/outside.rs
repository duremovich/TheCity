//! Life pass L2 phase 1 (spec § 1 "The export hook", plan L11): the World
//! account buys Food, Parts and Data; coins cross in and the M17 identity
//! `total_coins + Σ outside treasuries − minted` holds every day. The Real
//! economy (plan 1.2) adds the coin census: every source and sink of
//! `total_coins` outside `pay`/`charge`, booked per day on seeds 42-44.

use citysim::outside::ExportGood;
use citysim::systems::ownership;
use citysim::{Config, World, TICKS_PER_DAY};

fn identity(w: &World) -> i64 {
    ownership::total_coins(w) + w.outside.treasuries() - w.outside.minted
}

/// L2's flat-price hook (the economy off: with the market on the book's
/// pass replaces it, `tests/econ.rs`). The conservation identity's other
/// sources and sinks (immigrants' purses, emigrants' wallets, the fence)
/// move `total_coins` too: they are the documented ones, so the check is
/// the identity's drift against an export-off twin's `total_coins` drift
/// on the same seed's day.
#[test]
fn test_export_moves_goods_out_coins_in_and_identity_holds() {
    let mut cfg = Config::load().econ_off();
    cfg.export.enabled = true;
    // Low floors so every good sells on the first days.
    cfg.export.export_floor = 0;
    cfg.export.parts_floor = 0;
    cfg.export.data_floor = 0;
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::outside::export_on(&w));
    assert!(w.outside.is_empty(), "L2's hook creates the account lazily");
    let mut sold_days = 0;
    for _ in 0..4 {
        let before = identity(&w);
        let (minted, inbound) = (w.outside.minted, w.outside.inbound);
        // Run to just after the next midnight's export pass.
        let to_midnight = TICKS_PER_DAY - (w.tick % TICKS_PER_DAY);
        let living_before = ownership::total_coins(&w);
        w.run_ticks(to_midnight);
        // The export pass itself: coins in == minted refill drawn down.
        let crossed = w.outside.inbound - inbound;
        let refill = w.outside.minted - minted;
        if crossed > 0 {
            sold_days += 1;
        }
        // Across the day the identity moves only by the city's own
        // non-export sources and sinks, which move `total_coins` alike.
        let drift_identity = identity(&w) - before;
        let drift_coins = ownership::total_coins(&w) - living_before - crossed;
        assert_eq!(drift_identity, drift_coins, "the identity holds (refill {refill}, crossed {crossed})");
    }
    assert!(sold_days >= 2, "the World bought on most days");
    let sold = &w.outside.export.sold;
    for g in [ExportGood::Food, ExportGood::Parts, ExportGood::Data] {
        assert!(sold.get(&g).copied().unwrap_or(0) > 0, "{} went out: {sold:?}", g.label());
    }
    assert!(w.outside.inbound > 0 && w.outside.minted >= w.outside.inbound);
    let world = w.outside.faction(citysim::outside::WORLD_ACCOUNT).expect("the World account");
    assert_eq!(world.id, 1069);
    assert!(world.books.is_empty(), "L2's hook keeps no book");
    let flow: i64 = w.stats.history.iter().map(|r| r.living.flow_export).sum();
    assert_eq!(flow, w.outside.inbound, "flow_export books every crossing");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Exported));
}

/// With `[export]` and the economy off nothing crosses and `outside` stays
/// empty (and is not saved).
#[test]
fn test_export_off_outside_empty() {
    let mut w = World::new(42, Config::load().econ_off());
    assert!(!citysim::systems::outside::export_on(&w));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.outside.is_empty());
    assert!(w.stats.history.iter().all(|r| r.living.flow_export == 0));
    assert!(!citysim::save::to_ron(&w).contains("outside:"));
}

/// Real economy plan 1.2 (the census, E25): per day, `Δ total_coins` (plus
/// the loot in flight on open holes) minus the probe's booked sources and
/// sinks (immigrants' 15, emigrants' wallets, the fence's credit, loot lost
/// with an Unknown binding), the residue by seed; then the same seeds with
/// the market on, where every one of them is a crossing and the identity
/// must be constant. Seeds 42-44, 30 days; prints the table.
#[test]
#[ignore]
fn probe_coin_census() {
    let hole_loot = |w: &World| -> i64 { w.holes.values().map(|h| h.loot).sum() };
    let mut residues = Vec::new();
    let mut drifts = Vec::new();
    for seed in [42u64, 43, 44] {
        for on in [false, true] {
            let cfg = if on { Config::load() } else { Config::load().econ_off() };
            let mut w = World::new(seed, cfg);
            let base_coins = ownership::total_coins(&w) + if on { 0 } else { hole_loot(&w) };
            let base_ident = identity(&w);
            let mut residue = 0i64;
            let mut worst = (0u64, 0i64);
            let mut bad_days = Vec::new();
            let mut prev = (base_coins, w.probe.clone());
            for day in 0..30u64 {
                w.run_ticks(TICKS_PER_DAY);
                let coins = ownership::total_coins(&w) + if on { 0 } else { hole_loot(&w) };
                let p = w.probe.clone();
                let booked = (p.immigrant_coins - prev.1.immigrant_coins) - (p.emigrant_coins - prev.1.emigrant_coins)
                    + (p.fence_credit - prev.1.fence_credit)
                    - (p.loot_lost - prev.1.loot_lost);
                let delta = coins - prev.0;
                let r = if on { 0 } else { delta - booked };
                if on && identity(&w) != base_ident {
                    bad_days.push((day, identity(&w) - base_ident));
                }
                if r.abs() > worst.1.abs() {
                    worst = (day, r);
                }
                residue += r;
                prev = (coins, p);
            }
            let p = &w.probe;
            eprintln!(
                "census seed {seed} market {}: 30 days: immigrants +{} emigrants -{} fence +{} loot lost -{} (loot bound {}, \
                 in flight {}); Δ total_coins {}; residue {residue} (worst day {:?}); identity drift days {bad_days:?}",
                if on { "on" } else { "off" },
                p.immigrant_coins,
                p.emigrant_coins,
                p.fence_credit,
                p.loot_lost,
                p.loot_bound,
                hole_loot(&w),
                ownership::total_coins(&w) - base_coins + if on { 0 } else { hole_loot(&w) },
                worst
            );
            if on {
                drifts.push(bad_days);
            } else {
                residues.push(residue);
            }
        }
    }
    eprintln!("census residues (market off, 42-44) {residues:?}; identity drift days (market on) {drifts:?}");
    assert!(drifts.iter().all(|d| d.is_empty()), "with the market on the identity holds every day: {drifts:?}");
}
