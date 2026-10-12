//! Scale S3a bench (docs/SCALE_20K.md): the synthetic big island. Nine
//! copies of the shipped 256 × 192 city on a 3,000 × 1,875 street grid
//! (Road every 8th row and column, walled blocks between, a river with a
//! bridge every 200 columns across the south third), the El's lines copied
//! with each city, 24 districts (the shipped eight per column of cities).
//! Deterministic: the map is a function of the shipped map.
//!
//! `bench_big_island_paths` (ignored; run alone, release, `--nocapture`)
//! runs `CITYSIM_BENCH_POP` residents (default 6,000) for
//! `CITYSIM_BENCH_DAYS` days (3) with `[lod] max_full` = `CITYSIM_BENCH_FULL`
//! (50) and prints the path queries' latency (every Full walk's query,
//! recorded and replayed under a clock on the final map), the pathing
//! state's bytes, the process's peak working set and ticks/s.

use citysim::exec::bigpath;
use citysim::{Config, World, TICKS_PER_DAY};

const W: usize = 3_000;
const H: usize = 1_875;
/// The cities' origins, multiples of 8 so their road lines meet the grid's.
const OX: [usize; 3] = [0, 1_368, 2_744];
const OY: [usize; 3] = [0, 840, 1_680];
/// The river's rows: between the second and third rows of cities.
const RIVER: std::ops::Range<usize> = 1_400..1_407;

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// The island's map text and the per-copy offsets applied to the config.
fn island_text(cfg: &Config) -> String {
    let text = std::fs::read_to_string(cfg.asset(&cfg.world.map)).expect("the shipped map");
    let lines: Vec<&str> = text.lines().collect();
    let p: Vec<usize> = lines[0].split_whitespace().map(|s| s.parse().expect("header")).collect();
    let (w0, h0) = (p[0], p[1]);
    let tiles: Vec<Vec<char>> = lines[1..=h0].iter().map(|l| l.chars().collect()).collect();
    let zones: Vec<Vec<char>> = lines[h0 + 2..h0 + 2 + h0].iter().map(|l| l.chars().collect()).collect();
    let mut grid = vec![vec!['#'; W]; H];
    let mut zone = vec![vec!['U'; W]; H];
    for (y, row) in grid.iter_mut().enumerate() {
        for (x, t) in row.iter_mut().enumerate() {
            *t = if RIVER.contains(&y) {
                if x % 200 == 7 {
                    '='
                } else {
                    '~'
                }
            } else if x % 8 == 7 || y % 8 == 7 {
                '='
            } else if (x % 8 == 3 && y % 8 == 6) || ((1..6).contains(&(x % 8)) && (1..6).contains(&(y % 8))) {
                '.'
            } else {
                '#'
            };
        }
    }
    for &oy in &OY {
        for &ox in &OX {
            for y in 0..h0 {
                for x in 0..w0 {
                    grid[oy + y][ox + x] = tiles[y][x];
                    zone[oy + y][ox + x] = zones[y][x];
                }
            }
        }
    }
    let mut out = format!("{W} {H}\n");
    for row in &grid {
        out.extend(row.iter());
        out.push('\n');
    }
    out.push('\n');
    for row in &zone {
        out.extend(row.iter());
        out.push('\n');
    }
    out.push('\n');
    for &oy in &OY {
        for &ox in &OX {
            for line in &lines[h0 + 2 + h0..] {
                let p: Vec<&str> = line.split_whitespace().collect();
                if p.first() != Some(&"B") {
                    continue;
                }
                let x: usize = p[2].parse().expect("x");
                let y: usize = p[3].parse().expect("y");
                let rest = p[4..].join(" ");
                out.push_str(&format!("B {} {} {} {rest}\n", p[1], x + ox, y + oy));
            }
        }
    }
    out
}

fn island_config() -> Config {
    let mut cfg = Config::load();
    let path = std::env::temp_dir().join(format!("citysim-big-island-{}.txt", std::process::id()));
    std::fs::write(&path, island_text(&cfg)).expect("write the island");
    cfg.world.map = path.to_string_lossy().into_owned();
    let lines = std::mem::take(&mut cfg.transit.lines);
    for (j, &oy) in OY.iter().enumerate() {
        for (i, &ox) in OX.iter().enumerate() {
            for l in &lines {
                let mut l = l.clone();
                l.name = format!("{} {i}{j}", l.name);
                for p in &mut l.waypoints {
                    p[0] += ox as u16;
                    p[1] += oy as u16;
                }
                cfg.transit.lines.push(l);
            }
        }
    }
    let d = &mut cfg.districts;
    let (names, zones, from, to) = (
        std::mem::take(&mut d.names),
        std::mem::take(&mut d.zones),
        std::mem::take(&mut d.x_from),
        std::mem::take(&mut d.x_to),
    );
    for (c, &ox) in OX.iter().enumerate() {
        let (lo, hi) = (ox as u16, OX.get(c + 1).map_or(W, |&x| x) as u16);
        for i in 0..names.len() {
            d.names.push(format!("{} {c}", names[i]));
            d.zones.push(zones[i].clone());
            d.x_from.push(if from[i] == 0 { lo } else { from[i] + lo });
            d.x_to.push(if to[i] == 256 { hi } else { to[i] + lo });
        }
    }
    cfg
}

/// The process's peak working set, MB (Windows; 0 elsewhere).
#[cfg(windows)]
fn peak_mb() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_non_paged: usize,
        quota_non_paged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(process: *mut std::ffi::c_void, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Counters::default() };
    // SAFETY: a plain Win32 query into a correctly sized, owned struct.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 {
        0.0
    } else {
        c.peak_working_set as f64 / 1_048_576.0
    }
}

#[cfg(not(windows))]
fn peak_mb() -> f64 {
    0.0
}

#[test]
#[ignore]
fn bench_big_island_paths() {
    let pop = env("CITYSIM_BENCH_POP", 6_000) as u32;
    let days = env("CITYSIM_BENCH_DAYS", 3);
    let full = env("CITYSIM_BENCH_FULL", 50) as usize;
    let mut cfg = island_config().scaled_to(pop);
    cfg.lod.max_full = full;
    // `CITYSIM_BENCH_FLOW=1`: the comparison, main's flow fields (no
    // threshold, no byte budget: the 512-field LRU).
    let flow = env("CITYSIM_BENCH_FLOW", 0) == 1;
    if flow {
        cfg.exec.flow_field_max_tiles = usize::MAX;
        cfg.exec.flow_field_budget_mb = 1 << 20;
    }
    let t = std::time::Instant::now();
    let mut w = World::new(42, cfg);
    let build_s = t.elapsed().as_secs_f64();
    assert_eq!((w.map.w(), w.map.h()), (W, H));
    assert_eq!(w.big_paths(), !flow, "the island walks by A* paths");
    eprintln!(
        "island: {} residents, {} buildings, {} districts, {} El stops; world built in {build_s:.1} s, peak {:.0} MB",
        w.citizens().len(),
        w.map.buildings.len(),
        w.districts.len(),
        w.transit.net.stops.len(),
        peak_mb()
    );
    bigpath::record(true);
    let mut queries = Vec::new();
    for d in 0..days {
        let t = std::time::Instant::now();
        w.run_ticks(TICKS_PER_DAY);
        let s = t.elapsed().as_secs_f64();
        let q = bigpath::take_recorded();
        let row = w.stats.history.back().expect("a day");
        eprintln!(
            "day {d}: {:.0} ticks/s, {} path queries, {} flow fields ({:.0} MB), population {}, employed {}, Full {}, peak {:.0} MB",
            TICKS_PER_DAY as f64 / s,
            q.len(),
            w.flow_fields.len(),
            w.flow_fields.bytes() as f64 / 1_048_576.0,
            row.population,
            row.employed,
            row.tier_full,
            peak_mb()
        );
        queries.extend(q);
    }
    bigpath::record(false);
    // Replay every query on the final map under a clock: on 7 × Manhattan
    // alone and with the world's landmarks (what the run used).
    let labels = w.flow_fields.labels(&w.map);
    let marks = w.flow_fields.landmarks(&w.map);
    for (name, m) in [("Manhattan", bigpath::Landmarks::none()), ("ALT", (*marks).clone())] {
        let mut us: Vec<f64> = Vec::with_capacity(queries.len());
        let (mut steps, mut manhattan, mut found) = (0u64, 0u64, 0usize);
        let mut slow = Vec::new();
        let before = bigpath::expanded();
        for &(a, b) in &queries {
            let t = std::time::Instant::now();
            let p = bigpath::find(&w.map, &labels, &m, a, b);
            let q = t.elapsed().as_secs_f64() * 1e6;
            us.push(q);
            slow.push((q as u64, a, b, a.manhattan(b), p.as_ref().map_or(0, |p| p.len())));
            if let Some(p) = p {
                found += 1;
                steps += p.len() as u64;
                manhattan += u64::from(a.manhattan(b));
            }
        }
        let per = (bigpath::expanded() - before) / queries.len().max(1) as u64;
        slow.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        for (q, a, b, m, len) in slow.iter().take(3) {
            eprintln!("  {name} slow: {q} us {a} -> {b}, Manhattan {m}, {len} steps");
        }
        let total_ms = us.iter().sum::<f64>() / 1e3;
        us.sort_by(f64::total_cmp);
        let pct = |q: usize| us.get(us.len() * q / 100).copied().unwrap_or(0.0);
        eprintln!(
            "path queries ({name}): {} ({found} found), {per} expanded a query, {total_ms:.0} ms in all; latency p50 {:.0} us, p90 {:.0} us, p99 {:.0} us, max {:.0} us; steps / Manhattan {:.3}",
            us.len(),
            pct(50),
            pct(90),
            pct(99),
            us.last().copied().unwrap_or(0.0),
            steps as f64 / manhattan.max(1) as f64
        );
    }
    eprintln!(
        "pathing state: A* scratch {:.1} MB, labels + landmarks {:.1} MB, flow fields {} ({:.1} MB), El shortlist index {:.1} KB; process peak {:.0} MB",
        bigpath::scratch_bytes() as f64 / 1_048_576.0,
        w.flow_fields.index_bytes() as f64 / 1_048_576.0,
        w.flow_fields.len(),
        w.flow_fields.bytes() as f64 / 1_048_576.0,
        w.transit.net.near_bytes() as f64 / 1024.0,
        peak_mb()
    );
    assert!(flow || !queries.is_empty(), "Full walks asked for paths");
}
