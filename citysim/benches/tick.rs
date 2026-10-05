//! `bench_tick_300_agents`: CI fails if the median tick exceeds 125 µs.
//! `bench_tick_2000_agents` (M10): the v2 default city after a warm day; the
//! gate is a median under 250 µs (`tests/scale.rs`, release only).

use citysim::{Config, World, TICKS_PER_DAY};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tick_300_agents(c: &mut Criterion) {
    let mut world = World::new(42, Config::load().v1_profile());
    c.bench_function("tick_300_agents", |b| b.iter(|| world.tick()));
}

fn bench_tick_2000_agents(c: &mut Criterion) {
    let mut world = World::new(42, Config::load());
    // Warm one day: the first day builds the flow fields and fills the caches.
    world.run_ticks(TICKS_PER_DAY);
    c.bench_function("tick_2000_agents", |b| b.iter(|| world.tick()));
}

criterion_group!(benches, bench_tick_300_agents, bench_tick_2000_agents);
criterion_main!(benches);
