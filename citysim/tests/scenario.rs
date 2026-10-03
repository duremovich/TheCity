//! Long-running scenarios. The v1 acceptance test lands in M7 and is
//! `#[ignore]`d for CI with `--ignored`.

use citysim::{Config, World, TICKS_PER_DAY};

#[test]
fn test_m0_ten_days_headless() {
    let mut w = World::new(42, Config::load());
    for _ in 0..10 {
        w.run_ticks(TICKS_PER_DAY);
    }
    assert_eq!(w.stats.history.len(), 10);
    for (i, row) in w.stats.history.iter().enumerate() {
        assert_eq!(row.day, i as u64);
        assert_eq!(row.population, 300);
        assert_eq!(row.employed, 40);
        assert_eq!(row.homeless, 0);
        assert_eq!(row.price, 3);
        assert_eq!(row.treasury, 5000);
    }
    assert_eq!(w.stats.current.day, 10);
}
