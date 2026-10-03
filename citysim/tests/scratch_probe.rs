//! Ignored probes for by-hand investigation. Run one with
//! `cargo test --release -p citysim --test scratch_probe -- --ignored --nocapture <name>`.

use std::time::Instant;

use citysim::{Config, World, TICKS_PER_DAY};

/// Per-system wall time over one day after a long warm-up: where the ticks go.
#[test]
#[ignore]
fn probe_system_timing() {
    let warm_days: u64 = std::env::var("WARM_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(50);
    let mut w = World::new(42, Config::load());
    let t0 = Instant::now();
    w.run_ticks(warm_days * TICKS_PER_DAY);
    eprintln!("warm-up {warm_days} days in {:.1}s, edges {}", t0.elapsed().as_secs_f32(), w.edges.len());
    let names = [
        "commands", "lod", "needs", "memory", "mood", "think", "plan", "exec", "economy", "law", "social", "gang",
        "stats",
    ];
    let mut acc = [0f64; 13];
    for _ in 0..TICKS_PER_DAY {
        let steps: [&dyn Fn(&mut World); 13] = [
            &|w| w.apply_commands(),
            &citysim::systems::lod::run,
            &citysim::needs::run,
            &citysim::systems::memory::run,
            &citysim::mood::run,
            &citysim::systems::think::run,
            &citysim::systems::plan::run,
            &citysim::exec::run,
            &citysim::systems::economy::run,
            &citysim::systems::law::run,
            &citysim::systems::social::run,
            &citysim::systems::gang::run,
            &citysim::systems::stats::run,
        ];
        for (i, step) in steps.iter().enumerate() {
            let t = Instant::now();
            step(&mut w);
            acc[i] += t.elapsed().as_secs_f64();
        }
        w.tick += 1;
    }
    let total: f64 = acc.iter().sum();
    for (name, secs) in names.iter().zip(acc) {
        eprintln!("{name:>9} {:7.1} ms  {:5.1}%", secs * 1e3, secs / total * 100.0);
    }
    eprintln!("day total {:.0} ms -> {:.0} ticks/s", total * 1e3, TICKS_PER_DAY as f64 / total);
}
