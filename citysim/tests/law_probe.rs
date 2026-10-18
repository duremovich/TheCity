//! The law probe (Shadow V4 bugs 4 and 22, 2026-10-17; extended for Lives at
//! work phase 0, docs/LIVES_AT_WORK.md § 5.1, 2026-10-18): every witnessed
//! act (a `Witness` event, deduplicated per actor, crime and hour; the tier
//! the actor's LOD at the moment), reported within `REPORT_DAYS`, arrested
//! (an `Arrest` naming the actor as the suspect) within `FOLLOW_DAYS` and
//! sentenced (jailed or fined) within `FOLLOW_DAYS`, by tier and by crime;
//! the sentences by crime and the Precinct-full releases.
//!
//! § 5.1's question: does the arrest share fall with the suspect's walk to
//! the Precinct? Each act is binned by the actor's district (its `Position`
//! read right after the hour's `run_ticks(60)`, so up to an hour after the
//! act: good enough for a district) and the district's walk to the Precinct
//! in tiles (Manhattan from the district's centroid, its mean street tile,
//! to the Jail's door). One row per district sorted by that distance, then
//! the districts pooled into four bands by quartile of their distances (so
//! the bands adapt to the 20k map; each band prints its tile range), with
//! the rows by tier inside each band where the counts allow (rows under
//! `MIN_ROW_ACTS` acts are skipped, as the crime rows are).
//!
//! Runs: 2k seeds 42-44, a 20-day warm-up then 60 measured days each, pooled
//! and per seed; 20k seed 42, a 20-day warm-up then 30 measured days.
//! Printed `PROBE` lines, never asserted; the numbers are in
//! docs/SHADOW_V4.md (the V4 sweep) and docs/LIVES_AT_WORK.md § 5.1
//! (phase 0). `#[ignore = "tier: diag"]`:
//! `cargo test --release -p citysim --test law_probe -- --ignored --nocapture`
//! (`python tools/test.py diag --only law_probe_2k`). Env: `PROBE_SEEDS`
//! (comma-separated 2k seeds; `PROBE_SEED` for one), `PROBE_DAYS` (the
//! measured days of either run). The 20k run is ~10 min in release.
//!
//! Nothing here changes the run: no RNG draw, no component write, no event.
use citysim::{Building, BuildingKind, Config, EntityId, EventKind, Lod, Position, World, TICKS_PER_DAY};
use std::collections::{BTreeMap, BTreeSet};

/// The city runs this long before the measured window (the V4 recipe).
const WARMUP_DAYS: u64 = 20;
/// A report counts if it follows the act within a day.
const REPORT_DAYS: u64 = 1;
/// An arrest or a sentence counts if it follows the act within three days.
const FOLLOW_DAYS: u64 = 3;
/// A split row (a crime, a district, a tier in a band) under this many acts is not printed.
const MIN_ROW_ACTS: u32 = 20;
/// Distance bands over the districts' walks to the Precinct: quartiles.
const BANDS: usize = 4;

/// One witnessed act and what followed it.
#[derive(Clone)]
struct Act {
    crime: String,
    lod: Lod,
    /// The actor's district at the act (its name) and that district's walk to the Precinct.
    district: String,
    dist: u32,
    reported: bool,
    arrested: bool,
    /// `Some(fined)` when sentenced within `FOLLOW_DAYS`.
    sentenced: Option<bool>,
}

#[derive(Default, Clone, Copy)]
struct Row {
    acts: u32,
    reported: u32,
    arrested: u32,
    sentenced: u32,
    jailed: u32,
    fined: u32,
}

impl Row {
    fn add(&mut self, a: &Act) {
        self.acts += 1;
        self.reported += u32::from(a.reported);
        self.arrested += u32::from(a.arrested);
        if let Some(fined) = a.sentenced {
            self.sentenced += 1;
            if fined {
                self.fined += 1;
            } else {
                self.jailed += 1;
            }
        }
    }

    fn line(&self) -> String {
        let share = |n: u32| f64::from(n) / f64::from(self.acts.max(1));
        format!(
            "acts {} reported {:.2} arrested {:.2} sentenced {:.2} (jailed {:.2} fined {:.2})",
            self.acts,
            share(self.reported),
            share(self.arrested),
            share(self.sentenced),
            share(self.jailed),
            share(self.fined)
        )
    }
}

/// What one run measured: the acts with their outcomes, every district's
/// walk to the Precinct (name -> tiles), the sentence mix and the header line.
struct Run {
    acts: Vec<Act>,
    dist: BTreeMap<String, u32>,
    sent_kind: BTreeMap<String, u32>,
    header: String,
}

/// Run `w` for `days` more days and record every witnessed act with what followed.
fn probe(mut w: World, days: u64) -> Run {
    // (tick, actor, crime, tier, district index)
    let mut acts: Vec<(u64, EntityId, String, Lod, usize)> = Vec::new();
    let mut seen: BTreeSet<(EntityId, String, u64)> = Default::default();
    let mut reports: BTreeMap<EntityId, Vec<u64>> = BTreeMap::new();
    let mut arrests: BTreeMap<EntityId, Vec<u64>> = BTreeMap::new();
    let mut sentences: BTreeMap<EntityId, Vec<(u64, bool)>> = BTreeMap::new();
    let mut sent_kind: BTreeMap<String, u32> = BTreeMap::new();
    let mut cursor = 0u64;
    let end = w.tick + days * TICKS_PER_DAY;
    while w.tick < end {
        w.run_ticks(60);
        for e in w.events.iter().filter(|e| e.id >= cursor) {
            match e.kind {
                EventKind::Witness if e.actors.len() >= 2 => {
                    let a = e.actors[1];
                    let crime = e.text.rsplit(" commit ").next().unwrap_or("").to_string();
                    if seen.insert((a, crime.clone(), e.tick / 60)) {
                        let lod = w.comp::<citysim::Brain>(a).map_or(Lod::Statistical, |b| b.lod);
                        let d = w.comp::<Position>(a).map_or(0, |p| w.district_of(p.tile).index());
                        acts.push((e.tick, a, crime, lod, d));
                    }
                }
                EventKind::Report => {
                    if let Some(&a) = e.actors.first() {
                        reports.entry(a).or_default().push(e.tick);
                    }
                }
                // `[guard, suspect]` (`law::arrest`).
                EventKind::Arrest if e.actors.len() >= 2 => {
                    arrests.entry(e.actors[1]).or_default().push(e.tick);
                }
                EventKind::Sentence => {
                    if let Some(&a) = e.actors.first() {
                        let fined = e.text.contains(" fined ");
                        sentences.entry(a).or_default().push((e.tick, fined));
                        let k = if fined {
                            "fine".to_string()
                        } else {
                            e.text.rsplit(" for ").next().unwrap_or("?").to_string()
                        };
                        *sent_kind.entry(k).or_default() += 1;
                    }
                }
                EventKind::Unpunished => {
                    *sent_kind.entry("UNPUNISHED".to_string()).or_default() += 1;
                    if e.text.contains("Precinct full") {
                        *sent_kind.entry("UNPUNISHED (Precinct full)".to_string()).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
        cursor = w.events.back().map_or(cursor, |e| e.id + 1);
    }
    // Every district's walk to the Precinct: centroid to the Jail's door.
    let jail_door = w.building_of_kind(BuildingKind::Jail).and_then(|j| w.comp::<Building>(j)).map(|b| b.door);
    let dist: BTreeMap<String, u32> =
        w.districts.iter().map(|d| (d.name.clone(), jail_door.map_or(0, |door| d.centroid.manhattan(door)))).collect();
    let within = |v: Option<&Vec<u64>>, t: u64, days: u64| {
        v.is_some_and(|v| v.iter().any(|&x| x >= t && x <= t + days * TICKS_PER_DAY))
    };
    let acts: Vec<Act> = acts
        .into_iter()
        // An act too near the end has not had its `FOLLOW_DAYS`.
        .filter(|&(t, ..)| t + FOLLOW_DAYS * TICKS_PER_DAY <= end)
        .map(|(tick, actor, crime, lod, d)| {
            let district = w.districts.get(d).map_or_else(|| "?".to_string(), |x| x.name.clone());
            let dist = dist.get(&district).copied().unwrap_or(0);
            let sentenced = sentences
                .get(&actor)
                .and_then(|v| v.iter().find(|&&(x, _)| x >= tick && x <= tick + FOLLOW_DAYS * TICKS_PER_DAY))
                .map(|&(_, fined)| fined);
            Act {
                crime,
                lod,
                district,
                dist,
                reported: within(reports.get(&actor), tick, REPORT_DAYS),
                arrested: within(arrests.get(&actor), tick, FOLLOW_DAYS),
                sentenced,
            }
        })
        .collect();
    let header = format!(
        "population {}, guards {}, districts {}, days {days}, jailed now {} of capacity {}{}",
        w.citizens().len(),
        w.guards().len(),
        w.districts.len(),
        w.with::<citysim::Sentence>().len(),
        w.config.buildings.jail.capacity,
        if jail_door.is_none() { "; NO JAIL: distances read 0" } else { "" }
    );
    Run { acts, dist, sent_kind, header }
}

/// Print one run (or several pooled) under `label`.
fn report(label: &str, acts: &[Act], dist: &BTreeMap<String, u32>, sent_kind: Option<&BTreeMap<String, u32>>) {
    // By tier and crime (the V4 rows), plus the arrest column.
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for a in acts {
        for key in [format!("{:?}", a.lod), format!("{:?}/{}", a.lod, a.crime), "all".to_string()] {
            rows.entry(key).or_default().add(a);
        }
    }
    for (k, r) in &rows {
        if r.acts < MIN_ROW_ACTS && k.contains('/') {
            continue;
        }
        eprintln!("PROBE {label} {k}: {}", r.line());
    }
    if let Some(sk) = sent_kind {
        eprintln!("PROBE {label} sentences by crime: {sk:?}");
    }
    // By district, sorted by the walk to the Precinct.
    let mut by_district: BTreeMap<&str, Row> = BTreeMap::new();
    for a in acts {
        by_district.entry(a.district.as_str()).or_default().add(a);
    }
    let mut districts: Vec<(&str, u32)> = dist.iter().map(|(n, &d)| (n.as_str(), d)).collect();
    districts.sort_by_key(|&(n, d)| (d, n));
    for &(name, d) in &districts {
        let r = by_district.get(name).copied().unwrap_or_default();
        if r.acts < MIN_ROW_ACTS {
            continue;
        }
        eprintln!("PROBE {label} district {name} ({d} tiles to the Precinct): {}", r.line());
    }
    // Distance bands: quartiles of the districts' distances, each district in one band.
    if districts.is_empty() {
        return;
    }
    let ds: Vec<u32> = districts.iter().map(|&(_, d)| d).collect();
    // Cut points: the distance at each quartile of the sorted districts (inclusive upper bound).
    let cut = |q: usize| ds[((ds.len() * q) / BANDS).min(ds.len() - 1)];
    let cuts: Vec<u32> = (1..BANDS).map(cut).collect();
    let band_of = |d: u32| cuts.iter().position(|&c| d <= c).unwrap_or(BANDS - 1);
    let mut band_range: Vec<Option<(u32, u32)>> = vec![None; BANDS];
    let mut band_members: Vec<Vec<&str>> = vec![Vec::new(); BANDS];
    for &(name, d) in &districts {
        let b = band_of(d);
        band_range[b] = Some(band_range[b].map_or((d, d), |(lo, hi): (u32, u32)| (lo.min(d), hi.max(d))));
        band_members[b].push(name);
    }
    let mut band_rows: Vec<Row> = vec![Row::default(); BANDS];
    let mut band_tier: Vec<BTreeMap<String, Row>> = vec![BTreeMap::new(); BANDS];
    for a in acts {
        let b = band_of(a.dist);
        band_rows[b].add(a);
        band_tier[b].entry(format!("{:?}", a.lod)).or_default().add(a);
    }
    for b in 0..BANDS {
        let Some((lo, hi)) = band_range[b] else { continue };
        eprintln!(
            "PROBE {label} band {} ({lo}-{hi} tiles; {} district(s)): {}",
            b + 1,
            band_members[b].len(),
            band_rows[b].line()
        );
        for (tier, r) in &band_tier[b] {
            if r.acts < MIN_ROW_ACTS {
                continue;
            }
            eprintln!("PROBE {label} band {} {tier}: {}", b + 1, r.line());
        }
    }
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|s| s.parse().ok())
}

#[test]
#[ignore = "tier: diag"]
fn law_probe_2k() {
    let seeds: Vec<u64> = match std::env::var("PROBE_SEEDS") {
        Ok(s) => s.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
        Err(_) => env_u64("PROBE_SEED").map_or_else(|| vec![42, 43, 44], |s| vec![s]),
    };
    let days = env_u64("PROBE_DAYS").unwrap_or(60);
    // One thread per seed; printed in seed order afterwards.
    let runs: Vec<(u64, Run)> = std::thread::scope(|s| {
        let hs: Vec<_> = seeds
            .iter()
            .map(|&seed| {
                s.spawn(move || {
                    let mut w = World::new(seed, Config::load());
                    w.run_ticks(WARMUP_DAYS * TICKS_PER_DAY);
                    (seed, probe(w, days))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a law probe seed panicked")).collect()
    });
    for (seed, r) in &runs {
        let label = format!("2k s{seed}");
        eprintln!("PROBE {label}: {}", r.header);
        report(&label, &r.acts, &r.dist, Some(&r.sent_kind));
    }
    if runs.len() > 1 {
        let acts: Vec<Act> = runs.iter().flat_map(|(_, r)| r.acts.iter().cloned()).collect();
        // The 2k map is one file: every seed has the same districts and distances.
        let dist = &runs[0].1.dist;
        for (seed, r) in &runs[1..] {
            if &r.dist != dist {
                eprintln!("PROBE 2k pooled: seed {seed} has other district distances; the pooled bands read seed 42's");
            }
        }
        let label = format!("2k pooled s{:?}", seeds);
        eprintln!("PROBE {label}: {} acts over {} seeds x {days} days", acts.len(), runs.len());
        report(&label, &acts, dist, None);
    }
}

#[test]
#[ignore = "tier: diag"]
fn law_probe_20k() {
    let days = env_u64("PROBE_DAYS").unwrap_or(30);
    let mut c = Config::load().scaled_to(20_000);
    c.world.map = "gen:20000".into();
    c.ambient.flats_per_slot = 50.0;
    let mut w = World::new(42, c);
    w.run_ticks(WARMUP_DAYS * TICKS_PER_DAY);
    let r = probe(w, days);
    eprintln!("PROBE 20k s42: {}", r.header);
    report("20k s42", &r.acts, &r.dist, Some(&r.sent_kind));
}
