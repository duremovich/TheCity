//! Life pass L2 phase 1 (spec § 1 "The export hook", plan L11): the World
//! account buys Food, Parts and Data; coins cross in and the M17 identity
//! `total_coins + Σ outside treasuries − minted` holds every day.

use citysim::outside::ExportGood;
use citysim::systems::ownership;
use citysim::{Config, World, TICKS_PER_DAY};

fn identity(w: &World) -> i64 {
    ownership::total_coins(w) + w.outside.treasuries() - w.outside.minted
}

/// The conservation identity's other sources and sinks (immigrants'
/// purses, emigrants' wallets, the fence) move `total_coins` too: they
/// are the documented ones, so the check is the identity's drift against
/// an export-off twin's `total_coins` drift on the same seed's day.
#[test]
fn test_export_moves_goods_out_coins_in_and_identity_holds() {
    let mut cfg = Config::load();
    cfg.export.enabled = true;
    // Low floors so every good sells on the first days.
    cfg.export.export_floor = 0;
    cfg.export.parts_floor = 0;
    cfg.export.data_floor = 0;
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::outside::export_on(&w));
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
    let flow: i64 = w.stats.history.iter().map(|r| r.living.flow_export).sum();
    assert_eq!(flow, w.outside.inbound, "flow_export books every crossing");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Exported));
}

/// With `[export]` off nothing crosses and `outside` stays empty (and is
/// not saved).
#[test]
fn test_export_off_outside_empty() {
    let mut w = World::new(42, Config::load());
    assert!(!citysim::systems::outside::export_on(&w));
    w.run_ticks(3 * TICKS_PER_DAY);
    assert!(w.outside.is_empty());
    assert!(w.stats.history.iter().all(|r| r.living.flow_export == 0));
    assert!(!citysim::save::to_ron(&w).contains("outside:"));
}
