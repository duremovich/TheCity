# Lives at work phase 7 report: the 20k city (Stations, Spire homes)

Base 1810392 (main, SAVE 41). Branch: worktree agent-a441ff9fca1b74231. Final sha: **8117c92** (one commit on the branch, not merged).

## Built
- `BuildingKind::Station` (appended; city's, `[buildings.station]` capacity 16 seats, absent off), `systems/stations.rs` under `[law] stations` (`CITYSIM_STATIONS_OFF=1`): seed placement (farthest districts first; a Lot door >= `station_min_tiles` 240 from the Precinct AND from every Station placed), the seeded city guards posted (`Posts`: nearest post to Home among Stations under share + the Precinct), `reconcile_guards` hires through the same pick, `jail_for` (the `Jail` key: escort -> nearest of Precinct + Stations with a free cell; a Station guard -> his Station), per-post desk roster (`station_desk_share` 0.25), beats dealt to a Station's districts first then nearest-Home, booking at a Station (fine or a sentence ending before the next van served there), the 04:00 van (bodiless; `EventKind::Transferred`), `precinct_held` for every Precinct-full read (law, street, robots, riot), jailer encounter Precinct-only, a breakout frees only Precinct prisoners.
- Spire homes under `[life] exec_home_tier` 2 (`CITYSIM_SPIRE_HOMES_OFF=1`): `house_exec` already covered seed + promotion; added the daily `life::spire_homes` (skips an exec under a grown child's roof), the relocation tier floor for an exec, an exec's spouse and an exec swap partner, `spire_supply` FINDING. No eviction.
- Counters (Jobs2Cols): stations, station_bookings, station_fines, station_nights, transfers, stations_full, precinct_full, execs, execs_spire. Shadow `PROBE lives` line gains `travel_h/wk` and `spire n/m`. `core_20k` prints the stations FINDING and asserts a booking and a transfer. `tests/lod.rs` prints Station bookings per arrest by tier.
- SAVE_VERSION 42. Docs: LIVES_AT_WORK § 5.1 re-read table, § 13 20k after-rows, § 14 phase 7 block; ARCHITECTURE § 2, § 6, § 7. Also fixed a stray non-UTF-8 byte in § 14 phase 6 ("§ 4.2").
- Tests: `citysim/tests/stations.rs` (7 unit + `diag_spire_homes_20k`): placement, escort to nearest Station, full Station sends on, guard hired to Station nearest home, van transfers the long sentence (night served at the Station), exec in the Spire with no evictions, off cities write nothing.

## Placement measured
2k seed 42: 0 Stations at 240 (8 at min 1); 20k seed 42: 8 (756/668/652/588/500/412/396/324 tiles; 9 at 180 or 220).

## The device (20k, seed 42 x 30; "main" = tree with both switches off, proven identical to main at 20k 42 x 10)
law_probe_20k arrested (reported) by band 2-128 / 144-324 / 376-520 / 572-692 tiles:
| | band 1 | band 2 | band 3 | band 4 | all | UNPUNISHED |
|---|---|---|---|---|---|---|
| main | 0.83 (0.75) | 0.76 (0.50) | 0.56 (0.31) | 0.48 (0.25) | 0.71 (0.54) | 25 |
| phase 7 | 0.75 (0.64) | 0.70 (0.46) | 0.74 (0.47) | 0.73 (0.47) | 0.74 (0.55) | 0 |
Far bands up to the near ones (flat curve); near band down 0.83 -> 0.75. 2k share (phase 0) 0.73.

`run --population 20000 --scaled --map gen:20000 --days 30 --seed 42`, main binary -> phase 7:
| | main | phase 7 |
|---|---|---|
| arrests / thefts | 3,702 / 1,084 | 3,880 / 940 |
| deaths by violence / Murders | 238 / 184 | 244 / 189 |
| Precinct-full events | 0 | 0 |
| Station bookings (fines, nights) / transfers / sent on | - | 2,011 (761, 711) / 515 / 298 |
| execs in the Spire, day 30 | 85/110 (diag) | 101/106 (CSV); 103/109 (diag) |
| employed / pop | 12,182 / 19,883 | 12,299 / 19,885 |
Murders: an earlier phase build read 69 (main's late Sump 2 "assailant unknown" storm absent) and seed 43 x 30 read 63 -> 88 (the phase's storm): the fviolence storm variance, both directions.

Guards' travel (diaries, 80 guards, d20 x 7): mean 71.0 -> 67.9 h/wk (median 72.9 -> 68.2; Station-posted 22, mean 67.4). Lives probe (5 picks): 74.5 -> 66.5. Barely moved: the travel is the beat (patrol legs and chases are `travel`), not the commute: a Station guard living and posted in Mid 2 walks 8-13 h/day inside Mid 2. Flagged for the gate's diary read.
Execs (diag_spire_homes_20k, whole city, d0/10/20/30): main 89/90, 78/99, 83/105, 85/110 -> phase 7 89/90, 92/99, 99/104, 103/109. Spire supply at seed: 809 free tier-2 beds vs 135 wanted (execs x 1.5). Lives probe ceo/exec spire: 4/5, 4/5 -> 4/5, 5/5.
core_20k ticks/s (back to back): main 193 -> phase 7 196 (floor 140); bullets 1,929 bookings, 500 transfers; Murders 60 -> 43.

## 2k measure (tools/phase_measure.py, 42-44 x 120, base -> candidate; no Station at 2k)
| counter | 42 | 43 | 44 | mean |
|---|---|---|---|---|
| Murders | 126 -> 133 | 123 -> 136 | 133 -> 140 | 127 -> 136 |
| J24 | 0.759 -> 0.769 | 0.777 -> 0.764 | 0.715 -> 0.736 | 0.750 -> 0.756 |
| J25 | 0.016 -> 0.016 | 0.012 -> 0.015 | 0.020 -> 0.017 | 0.016 -> 0.016 |
| arrests | 2,769 -> 2,643 | 2,433 -> 2,523 | 2,488 -> 2,675 | 2,563 -> 2,614 |
| wallets | 79,529 -> 63,776 | 74,542 -> 77,703 | 91,163 -> 81,970 | 81,745 -> 74,483 |
| station counters | 0 | 0 | 0 | 0 |
| execs_spire / execs (day-sum) | 1,439/1,459 | 1,382/1,427 | 1,253/1,261 | |
Fresh seeds 45-47 x 120: Murders 71 -> 121, 103 -> 79, 36 -> 49 (mean 70 -> 83), wallets 87.5k -> 85.0k: the storm's spread both ways; no phase 7 path touches a kill roll (2k has no Station; only the exec Spire moves change the 2k city, a butterfly).
Device rows 2k (lives, 42-44 pooled): guard idle 4.26 -> 4.24, Served-able 0.70 -> 0.62; ceo/exec idle 7.17 -> 6.66.

## Off identity
- Both off, 2k seed 42 x 30: IDENTICAL (events 26,495 lines, 1,016 shared CSV columns, save with version masked).
- Both off, 20k seed 42 x 10: events byte-identical, 1,925 shared CSV columns identical (save not compared).
- Stations off alone: DIFFER by construction (Spire moves on). Spire off alone: events and CSV IDENTICAL at 2k, save DIFFERs (the stations' config keys).

## Behaviour test (cargo test --release -p citysim-cli -- --ignored behaviour)
PASS on the final tree (61.8 s). One row read near its bound: d19 fighter KnownMetWeek 2.85 (>= 2.8; main 3.55 with both switches off, also under its calibration range 4.70-8.63). The first build failed it (2.65): the exec/kin churn (an elder exec moved up daily, the kin take-in moved her back); fixed by `spire_homes` skipping an exec under a grown child's roof. Spire off alone reads main's 3.55: the row moves with the Spire butterfly, not with stations.

## Deviations (full list in LIVES_AT_WORK § 14 phase 7)
Spacing rule in placement; cells in `[law] station_cells` (not per capita); no new save store (held read off Position + Sentence); arrival at a just-filled Station books one over; seeded guards posted at seed; shares by residents nearest each post; Garrison holds each guard's own post; beats dealt nearest-Home for leftovers; spouse and swap-partner tier floors; kin-housed execs not moved; `guard_travel_ticks` not a CSV column (shadow `travel_h/wk` instead).

## Left
Gate: p_theft_caught regen (D14), god_cut_the_precinct, 42-47 runs, A/B tps, save size, § 9.1 diaries. Guards' travel is the beat (open question for the gate). Jail visits by kin still go to the Precinct for a Station-held prisoner (overnight only).

## Footers
- `python tools/test.py change`: `[change] GREEN; BUDGET OK 3m55s of 4m00s` (the first run was RED on `cli::test_cli_report_csv_header`: the header and column count gain the 9 new columns; fixed in the test's expected header, not loosened).
- `python tools/test.py lint`: `lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 52` (warnings pre-existing file sizes).
- Behaviour: `test result: ok. 1 passed` (fighter KnownMetWeek 2.85 >= 2.8; main 3.55).
- Off identity: IDENTICAL (both off, 2k 42 x 30; 20k 42 x 10 events + CSV).
