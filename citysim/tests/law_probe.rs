//! The law probe (Shadow V4 bugs 4 and 22, 2026-10-17): every witnessed act
//! (a `Witness` event, deduplicated per actor, crime and hour; the tier the
//! actor's LOD at the moment), reported within a day and sentenced (jailed
//! or fined) within three, by tier and by crime; the sentences by crime and
//! the Precinct-full releases. Printed `PROBE` lines, never asserted; the
//! numbers are in docs/SHADOW_V4.md's bug 22 hand-back. `#[ignore]`:
//! `cargo test --release -p citysim --test law_probe -- --ignored --nocapture`
//! (`PROBE_SEED` for the 2k seed; the 20k run is ~5 min).
use citysim::{Config, EntityId, EventKind, Lod, World, TICKS_PER_DAY};
use std::collections::BTreeMap;

fn probe(label: &str, mut w: World, days: u64) {
    #[derive(Default, Clone, Copy)]
    struct Row {
        acts: u32,
        reported: u32,
        sentenced: u32,
        jailed: u32,
        fined: u32,
    }
    // (tick, actor, crime, tier)
    let mut acts: Vec<(u64, EntityId, String, Lod)> = Vec::new();
    let mut seen: std::collections::BTreeSet<(EntityId, String, u64)> = Default::default();
    let mut reports: BTreeMap<EntityId, Vec<u64>> = BTreeMap::new();
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
                        acts.push((e.tick, a, crime, lod));
                    }
                }
                EventKind::Report => {
                    if let Some(&a) = e.actors.first() {
                        reports.entry(a).or_default().push(e.tick);
                    }
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
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for (t, a, crime, lod) in &acts {
        if *t + 3 * TICKS_PER_DAY > end {
            continue;
        }
        for key in [format!("{lod:?}"), format!("{lod:?}/{crime}"), "all".to_string()] {
            let r = rows.entry(key).or_default();
            r.acts += 1;
            if reports.get(a).is_some_and(|v| v.iter().any(|&x| x >= *t && x <= t + TICKS_PER_DAY)) {
                r.reported += 1;
            }
            if let Some(&(_, fined)) =
                sentences.get(a).and_then(|v| v.iter().find(|&&(x, _)| x >= *t && x <= t + 3 * TICKS_PER_DAY))
            {
                r.sentenced += 1;
                if fined {
                    r.fined += 1;
                } else {
                    r.jailed += 1;
                }
            }
        }
    }
    let pop = w.citizens().len();
    eprintln!(
        "PROBE {label}: population {pop}, guards {}, days {days}, jailed now {} of capacity {}",
        w.guards().len(),
        w.with::<citysim::Sentence>().len(),
        w.config.buildings.jail.capacity
    );
    for (k, r) in &rows {
        if r.acts < 20 && k.contains('/') {
            continue;
        }
        eprintln!(
            "PROBE {label} {k}: acts {} reported {:.2} sentenced {:.2} (jailed {:.2} fined {:.2})",
            r.acts,
            f64::from(r.reported) / f64::from(r.acts.max(1)),
            f64::from(r.sentenced) / f64::from(r.acts.max(1)),
            f64::from(r.jailed) / f64::from(r.acts.max(1)),
            f64::from(r.fined) / f64::from(r.acts.max(1)),
        );
    }
    eprintln!("PROBE {label} sentences by crime: {sent_kind:?}");
}

#[test]
#[ignore]
fn law_probe_2k() {
    let seed: u64 = std::env::var("PROBE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(42);
    let mut w = World::new(seed, Config::load());
    w.run_ticks(20 * TICKS_PER_DAY);
    probe(&format!("2k s{seed}"), w, 20);
}

#[test]
#[ignore]
fn law_probe_20k() {
    let mut c = Config::load().scaled_to(20_000);
    c.world.map = "gen:20000".into();
    c.ambient.flats_per_slot = 50.0;
    let mut w = World::new(42, c);
    w.run_ticks(20 * TICKS_PER_DAY);
    probe("20k s42", w, 10);
}
