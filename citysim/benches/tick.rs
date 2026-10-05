//! `bench_tick_300_agents`: CI fails if the median tick exceeds 125 µs.

use citysim::{Config, World};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tick_300_agents(c: &mut Criterion) {
    let mut world = World::new(42, Config::load().v1_profile());
    c.bench_function("tick_300_agents", |b| b.iter(|| world.tick()));
}

criterion_group!(benches, bench_tick_300_agents);
criterion_main!(benches);
