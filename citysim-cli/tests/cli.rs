//! M0: `run --days 1 --seed 1 --report` prints the exact header line (M15 W43: the word's columns;
//! Jobs & lives phase 1: the `[jobs2]` group before `ticks_per_sec`, the switch on as shipped; Lives at work
//! phase 7 appends its nine columns there; the pre-view pass phase 2 its nine `shift_*` after them; the perf pass
//! phase 5 the CLI's two wall-clock columns after `ticks_per_sec`, `max_tick_ms` and `max_tick_at`; phase 6 the
//! social graph's `edges`, `edges_new` and `coworker_pairs` after them; phase 7c `friends`).

use std::process::Command;

fn cli() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_citysim-cli"));
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

#[test]
fn test_cli_ten_days_seed_42() {
    let out = cli().args(["run", "--days", "10", "--seed", "42", "--report"]).output().expect("run cli");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let rows: Vec<&str> = stdout.lines().skip(1).collect();
    assert_eq!(rows.len(), 10);
    // M13 D20/D47 and M14 V14: a crash, an overdose and a Flatline are deaths
    // outside the three `deaths_*` columns.
    let header: Vec<&str> = stdout.lines().next().expect("header").split(',').collect();
    let other: Vec<usize> = ["crash_deaths", "overdoses", "flatlined"]
        .iter()
        .map(|c| header.iter().position(|h| h == c).expect("a death column"))
        .collect();
    let outside_cols: Vec<usize> =
        ["outsiders_arrived", "brothers_arrived"].iter().filter_map(|c| header.iter().position(|h| h == c)).collect();
    // M10: at 2,000 residents a birth or a death in ten days is ordinary, so the population is
    // checked as an account: 2,000 + births + immigrants - deaths - emigrants, with at most
    // thirteen immigrants a week from day 7 (M11 § 7: the lever times a logistic of Street
    // happiness).
    // The pressure pass S6 (OPEN-224): the opening account is the seed's
    // people, the 2,000 adults and the seeded couples' children (read off
    // day 0's row, less its own moves).
    let mut expected: Option<i64> = None;
    let mut immigrants = 0;
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<i64> =
            row.split(',').enumerate().filter(|&(k, _)| k != 1).map(|(_, c)| c.parse().unwrap_or(0)).collect();
        // cols (season dropped): 0 day, 1 population, ..., 13..=15 deaths, 16 births, 17 immigrants, 18 emigrants
        assert_eq!(cols[0], i as i64);
        let raw: Vec<&str> = row.split(',').collect();
        let other_deaths: i64 = other.iter().map(|&k| raw[k].parse::<i64>().unwrap_or(0)).sum();
        // M17 phase 3: agents from abroad (Outsiders, brothers) arrive outside the immigrant count.
        let abroad: i64 = outside_cols.iter().map(|&k| raw[k].parse::<i64>().unwrap_or(0)).sum();
        let net = cols[16] + cols[17] + abroad - cols[13] - cols[14] - cols[15] - cols[18] - other_deaths;
        let e = expected.get_or_insert(cols[1] - net);
        assert!(i > 0 || *e >= 2000, "the 2,000 adults at least: {e}");
        *e += net;
        immigrants += cols[17];
        assert_eq!(cols[1], *e, "day {i}");
        assert!(immigrants <= 13 * (i as i64 / 7), "day {i}");
        if i % 7 != 0 {
            assert_eq!(cols[17], 0, "immigrants arrive weekly: day {i}");
        }
    }
}

#[test]
fn test_cli_lever() {
    let out = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--events"])
        .args(["--lever", "day=0:release_reserve=200"])
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    // M10: three Markets fill during the day and farms overflow into the Warehouse, so its stock
    // is dynamic now; the release itself is checked through its event (split over 3 Markets).
    let events = String::from_utf8_lossy(&out.stderr);
    assert!(events.contains("Released 200 food from the Reserve Depot to 3 Street Markets"), "no release event");
}

#[test]
fn test_cli_bad_lever_exits_2() {
    let out = cli().args(["run", "--days", "1", "--seed", "1", "--lever", "nope"]).output().expect("run cli");
    assert_eq!(out.status.code(), Some(2));
}

/// M14 phase 4: every new lever parses and the plain ones apply (the
/// index-based god levers resolve against a live world: an index that names
/// nothing is skipped with a warning, not a parse error).
#[test]
fn test_cli_virt_levers_parse_and_apply() {
    let levers = [
        "day=1:city_ice=3",
        "day=1:data_tax=0.2",
        "day=1:hack_sentence=intrusion:9",
        "day=1:hack_sentence=data_theft:20",
        "day=1:wipe_data=0",
        "day=1:set_tech=0:chrome:2",
        "day=1:grant_data=0:deck:10",
        "day=1:grant_deck=0:2",
        "day=1:set_ice=0:2",
        "day=1:fry=0",
        "day=1:run_now=0:0:data",
        "day=1:run_now=0:corp0:ledger",
    ];
    let mut c = cli();
    c.args(["run", "--days", "3", "--seed", "42", "--events"]);
    for l in levers {
        c.args(["--lever", l]);
    }
    let out = c.output().expect("run cli");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {err}");
    for want in [
        "City ICE set to 3",
        "Data tax set to 20 %",
        "Sentence for Intrusion set to 9 days",
        "Sentence for Data Theft set to 20 days",
    ] {
        assert!(err.contains(want), "missing {want:?} in the events:\n{err}");
    }
    assert!(!err.contains("unknown lever") && !err.contains("expected <"), "a lever failed to parse:\n{err}");
}
