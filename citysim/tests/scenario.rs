//! Long-running scenarios. The v1 acceptance test lands in M7 and is
//! `#[ignore]`d for CI with `--ignored`.

use citysim::{Config, World, TICKS_PER_DAY};

/// `run --days 30 --seed 42`: the M1 gate. Hunger stays up, nobody much
/// starves, the price stays sane and the Market is not empty for long.
#[test]
fn test_m1_thirty_days_seed_42() {
    let mut w = World::new(42, Config::load());
    for _ in 0..30 {
        w.run_ticks(TICKS_PER_DAY);
    }
    assert_eq!(w.stats.history.len(), 30);
    let mut empty_streak = 0;
    let mut starvation = 0;
    for row in &w.stats.history {
        assert!(row.mean_hunger >= 0.4, "day {}: mean_hunger {}", row.day, row.mean_hunger);
        assert!((2..=8).contains(&row.price), "day {}: price {}", row.day, row.price);
        starvation += row.deaths_starvation;
        // M2: goal selection should neither flap nor stick. Day 0 is excused:
        // everyone starts fed and solvent, so there is little to change one's mind about.
        assert!(
            row.day == 0 || (3.0..=12.0).contains(&row.goal_changes_per_agent),
            "day {}: goal_changes_per_agent {}",
            row.day,
            row.goal_changes_per_agent
        );
        if row.food_market == 0 {
            empty_streak += 1;
            assert!(empty_streak <= 2, "day {}: Market empty for {empty_streak} days", row.day);
        } else {
            empty_streak = 0;
        }
    }
    assert!(starvation <= 5, "{starvation} starvation deaths");
}

#[test]
fn test_m0_ten_days_headless() {
    let mut w = World::new(42, Config::load());
    for _ in 0..10 {
        w.run_ticks(TICKS_PER_DAY);
    }
    assert_eq!(w.stats.history.len(), 10);
    for (i, row) in w.stats.history.iter().enumerate() {
        assert_eq!(row.day, i as u64);
        assert!(row.population >= 295, "day {}: population {}", row.day, row.population);
        assert_eq!(row.homeless, 0);
        assert!((1..=30).contains(&row.price));
    }
    assert_eq!(w.stats.current.day, 10);
}
