//! M0: `run --days 1 --seed 1 --report` prints the exact header line.

use std::process::Command;

fn cli() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_citysim-cli"));
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

#[test]
fn test_cli_report_csv_header() {
    let out = cli().args(["run", "--days", "1", "--seed", "1", "--report"]).output().expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next(),
        Some("day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,acquisitions,bankruptcies,monopolies,ticks_per_sec")
    );
    let row = lines.next().expect("one data row");
    // M10: 2,000 residents, 220 jobs (160 / 36 / 12 / 8 / 4), nobody homeless;
    // M11: plus 12 private guards hired at tick 0 (two Security Offices) and
    // a bartender for the agent-owned Bar short of its staff of 3.
    // (Off-screen thieves can be jailed on day 0 since M10 phase 3, and an
    // off-screen killing can take a resident, and their job, on day 0.)
    let cols: Vec<&str> = row.split(',').collect();
    assert_eq!(&cols[..2], ["0", "Spring"], "row: {row}");
    let n = |i: usize| cols[i].parse::<u32>().expect("a count");
    assert!((1995..=2000).contains(&n(2)) && (215..=236).contains(&n(3)) && n(4) == 0, "row: {row}");
    assert_eq!(row.split(',').count(), 71);
    assert!(lines.next().is_none());
}

#[test]
fn test_cli_ten_days_seed_42() {
    let out = cli().args(["run", "--days", "10", "--seed", "42", "--report"]).output().expect("run cli");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let rows: Vec<&str> = stdout.lines().skip(1).collect();
    assert_eq!(rows.len(), 10);
    // M10: at 2,000 residents a birth or a death in ten days is ordinary, so the population is
    // checked as an account: 2,000 + births + immigrants - deaths - emigrants, with thirteen
    // immigrants a week from day 7.
    let mut expected: i64 = 2000;
    let mut immigrants = 0;
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<i64> =
            row.split(',').enumerate().filter(|&(k, _)| k != 1).map(|(_, c)| c.parse().unwrap_or(0)).collect();
        // cols (season dropped): 0 day, 1 population, ..., 13..=15 deaths, 16 births, 17 immigrants, 18 emigrants
        assert_eq!(cols[0], i as i64);
        expected += cols[16] + cols[17] - cols[13] - cols[14] - cols[15] - cols[18];
        immigrants += cols[17];
        assert_eq!(cols[1], expected, "day {i}");
        assert_eq!(immigrants, 13 * (i as i64 / 7), "day {i}");
    }
}

#[test]
fn test_cli_lever_and_save_at() {
    let dir = std::env::temp_dir().join(format!("citysim-cli-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--events"])
        .args(["--lever", "day=0:release_reserve=200", "--save-at", "100"])
        .arg("--saves-dir")
        .arg(&dir)
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let row = stdout.lines().nth(1).expect("row");
    // M10: three Markets fill during the day and farms overflow into the Warehouse, so its stock
    // is dynamic now; the release itself is checked through its event (split over 3 Markets).
    let events = String::from_utf8_lossy(&out.stderr);
    assert!(events.contains("Released 200 food from the Reserve Depot to 3 Street Markets"), "no release event");
    assert!(dir.join("3-100.ron").is_file());

    // resume from the save and finish the day: identical row
    let out2 = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--load"])
        .arg(dir.join("3-100.ron"))
        .output()
        .expect("run cli");
    assert!(out2.status.success(), "stderr: {}", String::from_utf8_lossy(&out2.stderr));
    let stdout2 = String::from_utf8(out2.stdout).expect("utf8");
    let row2 = stdout2.lines().nth(1).expect("row");
    let strip = |r: &str| r.rsplit_once(',').map(|(a, _)| a.to_string()).expect("row");
    assert_eq!(strip(row2), strip(row));
    // replaying with the same --lever must not double-apply the lever already in the save
    let out3 = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--lever", "day=0:release_reserve=200", "--load"])
        .arg(dir.join("3-100.ron"))
        .output()
        .expect("run cli");
    assert!(out3.status.success());
    let stderr3 = String::from_utf8_lossy(&out3.stderr);
    assert!(stderr3.contains("skipped"), "stderr: {stderr3}");
    let stdout3 = String::from_utf8(out3.stdout).expect("utf8");
    assert_eq!(strip(stdout3.lines().nth(1).expect("row")), strip(row));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cli_save_at_end_tick_and_out_of_range() {
    let dir = std::env::temp_dir().join(format!("citysim-cli-test-end-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = cli()
        .args(["run", "--days", "1", "--seed", "4", "--save-at", "1440", "--saves-dir"])
        .arg(&dir)
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("4-1440.ron").is_file(), "save at the final tick must be written");
    let out = cli()
        .args(["run", "--days", "1", "--seed", "4", "--save-at", "1441", "--saves-dir"])
        .arg(&dir)
        .output()
        .expect("run cli");
    assert_eq!(out.status.code(), Some(2), "out-of-range --save-at must fail loudly");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cli_bad_lever_exits_2() {
    let out = cli().args(["run", "--days", "1", "--seed", "1", "--lever", "nope"]).output().expect("run cli");
    assert_eq!(out.status.code(), Some(2));
}
