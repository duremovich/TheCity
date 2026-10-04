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
        Some("day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,ticks_per_sec")
    );
    let row = lines.next().expect("one data row");
    assert!(row.starts_with("0,Spring,300,40,0,0,0,"), "row: {row}");
    assert_eq!(row.split(',').count(), 25);
    assert!(lines.next().is_none());
}

#[test]
fn test_cli_ten_days_seed_42() {
    let out = cli().args(["run", "--days", "10", "--seed", "42", "--report"]).output().expect("run cli");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let rows: Vec<&str> = stdout.lines().skip(1).collect();
    assert_eq!(rows.len(), 10);
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<&str> = row.split(',').collect();
        assert_eq!(cols[0], i.to_string());
        // Two immigrants a week from day 7 (M6); nobody dies in the first ten days.
        let expected = 300 + 2 * (i / 7);
        assert_eq!(cols[2], expected.to_string());
    }
}

#[test]
fn test_cli_lever_and_save_at() {
    let dir = std::env::temp_dir().join(format!("citysim-cli-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report"])
        .args(["--lever", "day=0:release_reserve=200", "--save-at", "100"])
        .arg("--saves-dir")
        .arg(&dir)
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let row = stdout.lines().nth(1).expect("row");
    let cols: Vec<&str> = row.split(',').collect();
    // 1500 - 200 released, then the day-0 1% spoilage: 1287. Market stock is dynamic.
    assert_eq!(cols[8], "1287", "warehouse stock after release and spoilage");
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
