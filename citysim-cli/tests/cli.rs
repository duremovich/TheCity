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
        Some("day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,corp9_treasury,corp9_order,acquisitions,bankruptcies,monopolies,foundings,incorporations,strikes,unrest_corp,unrest_street,unrest_dreg,class_corp,class_street,class_dreg,happiness_street,d1_coverage,d1_control,d1_litter,d1_unrest,d1_crime,d1_guards,d2_coverage,d2_control,d2_litter,d2_unrest,d2_crime,d2_guards,d3_coverage,d3_control,d3_litter,d3_unrest,d3_crime,d3_guards,d4_coverage,d4_control,d4_litter,d4_unrest,d4_crime,d4_guards,d5_coverage,d5_control,d5_litter,d5_unrest,d5_crime,d5_guards,d6_coverage,d6_control,d6_litter,d6_unrest,d6_crime,d6_guards,d7_coverage,d7_control,d7_litter,d7_unrest,d7_crime,d7_guards,d8_coverage,d8_control,d8_litter,d8_unrest,d8_crime,d8_guards,hotel_nights,squatters,derelicts,vagrancy,riots,crossfire,gangs,vehicles_moto,vehicles_car,vehicles_truck,vehicles_flyer,truck_hauls,walk_hauls,commute_tpt_walk,commute_tpt_drive,chrome_installs,chrome_agents,mean_sanity,episodes,hooked,stims_dealt,stims_legal,dealing_reports,repos,impounds,crashes,crash_deaths,vehicle_thefts,chops,abductions,stripped,robots,flow_asset,flow_asset_upkeep,flow_finance,flow_import,flow_stims,flow_parts,flow_treatment,overdoses,harvests,stripped_window,gang_income,gang_income_dealing,episodes_by_law,treatments,detoxes,nodes,labs,decks,runs,runs_ok,data_made,data_stolen,data_wiped,data_sold,ledger_hacks,doors_hacked,traced,fried,flatlined,hack_arrests,ice_mean_corp,ice_spend,runs_bounced,runs_captured,runs_dumped,hack_arrests_chair,robots_turned,blinded,cameras,sightings,ice_raised,ice_lowered,tech_gained,tech_lost,research_spent,data_held,corp1_tier_chrome,corp1_tier_deck,corp1_tier_industry,corp1_data,corp2_tier_chrome,corp2_tier_deck,corp2_tier_industry,corp2_data,corp3_tier_chrome,corp3_tier_deck,corp3_tier_industry,corp3_data,corp4_tier_chrome,corp4_tier_deck,corp4_tier_industry,corp4_data,corp5_tier_chrome,corp5_tier_deck,corp5_tier_industry,corp5_data,corp6_tier_chrome,corp6_tier_deck,corp6_tier_industry,corp6_data,corp7_tier_chrome,corp7_tier_deck,corp7_tier_industry,corp7_data,corp8_tier_chrome,corp8_tier_deck,corp8_tier_industry,corp8_data,corp9_tier_chrome,corp9_tier_deck,corp9_tier_industry,corp9_data,flow_data,flow_hack,flow_ice_upkeep,flow_research,flow_terminal,ticks_per_sec")
    );
    let row = lines.next().expect("one data row");
    // M10: 2,000 residents, 220 jobs (160 / 36 / 12 / 8 / 4), nobody homeless;
    // M11: plus 12 private guards hired at tick 0 (two Security Offices) and
    // a bartender for the agent-owned Bar short of its staff of 3.
    // (Off-screen thieves can be jailed on day 0 since M10 phase 3, and an
    // off-screen killing can take a resident, and their job, on day 0.)
    // M12 D26: the seeded derelicts' residents start homeless. M12 D23: the
    // first midnight hires 5 sweepers (the reconcile's daily cap): 220 + 12 +
    // 1 + 5 = 238 on seeds 1-3, less a resident jailed or killed off-screen
    // (fix pass: the bound 250 tightened to 230-240). M13 D18: plus up to
    // six Mechanics at the two seeded Garages, and (phase 3) up to six
    // Ripperdocs at the three seeded Clinics (230-252). M13 phase 5: the
    // Tech corp's day-0 Garage hires three more Mechanics (9 in all on seed
    // 1), and with nobody jailed or killed off screen that day seed 1 read
    // 253: 230-255. M14 V16: plus the four seeded Labs' sixteen Researchers
    // hired at the first midnight: 230-271.
    let cols: Vec<&str> = row.split(',').collect();
    assert_eq!(&cols[..2], ["0", "Spring"], "row: {row}");
    let n = |i: usize| cols[i].parse::<u32>().expect("a count");
    let derelict_homeless = 5 * citysim::Config::load().street.seed_derelict_blocks as u32;
    assert!((1995..=2000).contains(&n(2)) && (230..=271).contains(&n(3)), "row: {row}");
    assert!((derelict_homeless - 5..=derelict_homeless).contains(&n(4)), "row: {row}");
    // M13 D49: corp slot 9 and the 40 asset columns; M14 V43: the 72 Virt columns.
    assert_eq!(row.split(',').count(), 81 + 8 * 6 + 7 + 2 + 40 + 72);
    assert!(lines.next().is_none());
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
    // M10: at 2,000 residents a birth or a death in ten days is ordinary, so the population is
    // checked as an account: 2,000 + births + immigrants - deaths - emigrants, with at most
    // thirteen immigrants a week from day 7 (M11 § 7: the lever times a logistic of Street
    // happiness).
    let mut expected: i64 = 2000;
    let mut immigrants = 0;
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<i64> =
            row.split(',').enumerate().filter(|&(k, _)| k != 1).map(|(_, c)| c.parse().unwrap_or(0)).collect();
        // cols (season dropped): 0 day, 1 population, ..., 13..=15 deaths, 16 births, 17 immigrants, 18 emigrants
        assert_eq!(cols[0], i as i64);
        let raw: Vec<&str> = row.split(',').collect();
        let other_deaths: i64 = other.iter().map(|&k| raw[k].parse::<i64>().unwrap_or(0)).sum();
        expected += cols[16] + cols[17] - cols[13] - cols[14] - cols[15] - cols[18] - other_deaths;
        immigrants += cols[17];
        assert_eq!(cols[1], expected, "day {i}");
        assert!(immigrants <= 13 * (i as i64 / 7), "day {i}");
        if i % 7 != 0 {
            assert_eq!(cols[17], 0, "immigrants arrive weekly: day {i}");
        }
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
