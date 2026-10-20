# Shadow V6, phases 0-1: the tool and the windows (runs report, 2026-10-20)

Branch `worktree-agent-a64b0601941c2715f`, tool commit `f81289e` on `8281474` (`a25fdc5` + the brief). Diaries stay in the worktree for the critics; every path below is absolute.

**Phase 0** (`citysim-cli/src/shadow.rs` only; `main.rs` untouched, `--map` reuses its `map_path`). Each diary opens with `## Camera (Shadow V6)`; each window writes `camera.tsv` (a row per pick and day). Jumps and settlements apart also print as `[camera]` lines in the day; parcels as `[parcel]` lines. Definitions (module header):
- **place**: held (a `Sentence`) > stat > coarse > vehicle (`Ride` incl. the platform, `Fly`, a `Goto` with `Kit.driving`) > home > work (employer, an exec's Hq, or a work/Meeting step anywhere) > another building > street.
- **jump**: a change of the body's tile (its building's door while inside) by more than the Chebyshev allowance accrued in travel states since the last change (walk 1/tick, drive `max_steps_per_tick`, flyer `1/flyer_ticks_per_tile`, El `1/ride_ticks_per_tile`, a Coarse timed walk 1) plus 2 tiles. "body" = Full and free at both ends.
- **still**: the longest run of one (tile, building, step, exec state), credited to the day it began; "awake" excludes sleep-class stretches.
- **settlements**: flow notes and `Served` memories naming a person; *apart* = the counterpart a body not co-located (same building, or both outdoors within 2 tiles), *stat* = the counterpart Statistical, *asleep* = the pick asleep.
- **labels**: a `Served` line names kind and side (`served X (a customer)`, `was called on by X (as a tenant; my super)`, `collected from X (my street worker)`, `sat at X's table (a Meeting)`, `handed a parcel to X (the recipient)`, ...). The memory carries neither, so the side comes from the regulars ledger (stamped today), then the pimp's pitch, then the pair's `[[serves]]` rows; an unresolved pair prints `served-pair ... (kind and side unknown: my row R, theirs S)` (none left in the smoke test). The Camp/Jail count is never "a regular"; the school fee is `SchoolFee`; `meeting` is a time-use class.
- **Left out / limits**: no column needed a sim hook. A cab *passenger* is not counted as "vehicle" (no exec state marks one; `jobs2.cab_rides` could, in the tool, not done). The super's Block stop can overflow the regulars ledger's cap in one tick, so tenant sides there come from the rows (correct for super vs non-super pairs).
- **Guard**: off identity on seed 42 x 30 vs `8281474` (`phase_measure.py --off CITYSIM_SHADOW_V6_OFF`, a no-op switch): events identical (26,480 lines), report CSV 1,027 cols x 30 rows identical, save identical (save_version 45 and ticks_per_sec masked). `test.py change`: **GREEN**, 4m45s of 4m00s (budget breached; other agents' builds on the box; the phase adds two sub-second tests).

## Phase 1: commands and wall times

`B=target/shadow_v6/bin/citysim-cli.exe` (the `f81289e` release build), `A` = V5's 48-archetype list (`SHADOW_V5.md`). Five jobs ran in parallel, so wall times (s) are loaded-box times. Script: `target/shadow_v6/run_windows.sh`.

| window | command | wall | diaries | archetypes with no candidate |
|---|---|---|---|---|
| `k20_s42_d0` | `shadow --population 20000 --seed 42 --start-day 0 --days 7 --count 2 --pick $A` (+ `--list`: 25 s) | 82 | 58 | **19**: gang_member, gang_leader, runner, purist, dealer, sweeper, gun, buyer, joytoy, street_worker, pimp, vendor, day_labourer, office_worker, arms_dealer, companion, black_doc, cultist, scav |
| `k20_s43_d30` | `... --seed 43 --start-day 30 ...` (`--list` 294 s) | 422 | 94 | black_doc |
| `k20_s44_d90` | `... --seed 44 --start-day 90 ...` (`--list` 995 s) | 1022 | 92 | black_doc; 1 street_worker, 1 arms_dealer |
| `k2_s42_d26` | `shadow --seed 42 --start-day 26 --days 7 --count 1 --pick $A` | 14 | 42 | dealer, buyer, street_worker, pimp, arms_dealer, black_doc |
| `k20_s43_d30_nopin` | `shadow --population 20000 --seed 43 --start-day 30 --days 3 --count 1 --no-pin --pick worker,homeless,gang_member,cook,courier,cabbie,dancer,guard,office_worker,street_worker,vendor,elder` | 335 | 12 | - |
| `isle_s42` | `run --seed 42 --population 20000 --scaled --map assets/maps/isle_m_k4.txt --ambient-flats 50 --days 30 --report --events`; then `shadow --population 20000 --map assets/maps/isle_m_k4.txt --seed 42 --start-day 30 --list` | 234 + 205 | - | dealer, joytoy, office_worker, arms_dealer, madam, companion, black_doc, teacher |

Day 0 at 20k: worker_friday starts on weekday 0, not its Friday (the tool notes it). Day 0 has 800 homeless, 5,214 elders of 20,000 and no gang of any kind.

Diary directories (`.md` + `.jsonl` per pick, `camera.tsv`, the window's stdout in `<window>.log` beside it):
- `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\k20_s42_d0`
- `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\k20_s43_d30`
- `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\k20_s44_d90`
- `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\k2_s42_d26`
- `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\k20_s43_d30_nopin`
- island probe (no diaries): `D:\TheCity\.claude\worktrees\agent-a64b0601941c2715f\target\shadow_v6\isle_s42` (`run.csv`, `events.tsv`), its list in `isle_s42_list.log`
- per-window camera roll-ups: `...\target\shadow_v6\camera_rollup.md` (every archetype, every window, all columns), `camera_pooled.md` (below)

## The camera, per window (means per pick-day; hours unless marked)

| window | pick-days | home | work | bldg | street | veh | held | stat | coarse | plans | aborts | jumps (body) | awake still min | settles (apart/stat/asleep) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| k20_s42_d0 | 406 | 9.1 | 5.7 | 2.1 | 6.2 | 0.3 | 0.6 | 0 | 0 | 11.0 | 1.8 | 5 (0) total | 386 | 3.4 (0.6/2.2/0.3) |
| k20_s43_d30 | 654 | 8.4 | 5.1 | 1.5 | 7.5 | 0.4 | 1.1 | 0 | 0 | 9.9 | 1.7 | 15 (3) total | 413 | 3.8 (0.2/2.7/0.1) |
| k20_s44_d90 | 640 | 7.9 | 5.0 | 2.2 | 7.3 | 0.6 | 1.0 | 0 | 0 | 10.0 | 1.7 | 14 (5) total | 371 | 3.4 (0.2/2.9/0.2) |
| k2_s42_d26 | 294 | 7.8 | 4.5 | 1.9 | 7.2 | 0.3 | 2.3 | 0 | 0 | 10.0 | 1.7 | 9 (0) total | 453 | 3.9 (0.2/3.2/0.1) |
| nopin | 36 | 1.1 | 0.1 | 0.1 | 0.8 | 0 | 0 | 16.0 | 6.0 | 3.8 | 0.8 | 1 (1) total | 1049 | 1.3 (0.1/1.1/0) |

- **Aborts** by reason, pooled pinned windows (3,353): Interrupted 2,838 (85 %), Unplannable 228, PreconditionLost 173, NoSuchPlace 42, BuildingFull 30, PartnerLeft 29, StockGone 13.
- **Jumps** (44 total): 35 are the arrest (29 `LOD Full -> Coarse, sentenced`, 6 `held`: the body appears at the Precinct or a Station). The 9 body jumps: 4 guards and 1 runner arriving at a Station or the Precinct 65-200 tiles away in 72-83 min (a booking, escort or transfer faster than the walk: `k20_s44_d90/guard_20192` d90 23:03, 200 tiles), and 3 CEO arrivals at the Hq plus 1 runner at a Ripperdoc for a Meeting step, 113-688 tiles at ~1.1x the flyer allowance (an allowance artefact or a faster flyer kind; unchecked).
- **Settlements**: 76 % of the counterparts named are Statistical (5,418 of 7,153, off screen); body-apart 8 %; the pick asleep 10 % at 20k d0 (142 of 1,396) against 1-5 % later. The supers' tenants, the CEOs' and execs' tables and the buyers' sellers are almost all Statistical.
- **nopin**: 8 of 12 unpinned picks were Statistical all 3 days, one unchanged stretch a day (awake still 1,440 min: the body stands at its snapshot); the cabbie, gang member and homeless pick were Coarse all 3 days (no jump: the timed walks fit the allowance); only the guard was Full (and made the one body jump).

## The camera, pooled over the four pinned windows, by archetype (per pick-day)

| arch | pd | street h | veh h | held h | plans | aborts | awake still m | settles | apart | stat | asleep |
|---|---|---|---|---|---|---|---|---|---|---|---|
| arms_dealer | 21 | 2.7 | 0.3 | 19.7 | 1.0 | 0.3 | 1214 | 1.0 | 0.0 | 0.3 | 0.0 |
| bouncer / builder / buyer | 49 / 49 / 28 | 6.9 / 6.1 / 2.3 | 0.2 / 0.2 / 0.1 | 1.4 / 2.1 / 0 | 12.0 / 7.9 / 11.4 | 2.0 / 0.9 / 1.5 | 366 / 506 / 358 | 1.1 / 3.5 / 18.4 | 0.1 / 1.6 / 0.1 | 0.5 / 1.2 / 17.9 | 0 / 0.8 / 0.2 |
| cabbie / camp_warden / ceo | 49 / 49 / 49 | 6.0 / 5.3 / 4.4 | 0.2 / 0.1 / 0.1 | 0 / 0 / 2.9 | 13.6 / 9.4 / 8.4 | 1.8 / 1.1 / 1.3 | 135 / 467 / 558 | 0.8 / 3.8 / 28.8 | 0.2 / 0 / 0.3 | 0.3 / 0.8 / 23.9 | 0 / 0 / 0.1 |
| club_staff / companion / cook | 49 / 35 / 49 | 6.1 / 8.4 / 5.9 | 0.1 / 0 / 0.1 | 0 / 1.1 / 0 | 12.6 / 14.3 / 12.0 | 2.6 / 2.1 / 2.1 | 279 / 330 / 338 | 1.4 / 2.5 / 0.8 | 0 / 0 / 0.1 | 1.1 / 2.4 / 0.5 | 0 |
| courier / cultist / dancer | 49 / 35 / 49 | 9.3 / 7.0 / 8.1 | 0.4 / 0.9 / 0.4 | 0 / 5.3 / 0 | 10.5 / 7.8 / 12.7 | 1.3 / 2.3 / 2.3 | 277 / 566 / 278 | 1.2 / 1.7 / 3.3 | 0 / 0.2 / 0 | 0.9 / 1.0 / 3.0 | 0 |
| day_labourer / dealer / elder | 35 / 28 / 49 | 11.1 / 7.3 / 9.9 | 0.4 / 1.3 / 0.5 | 0 / 3.7 / 0 | 10.7 / 7.1 / 9.5 | 1.3 / 1.6 / 1.4 | 248 / 532 / 359 | 1.2 / 1.4 / 0.8 | 0 | 0.9 / 0.8 / 0.4 | 0 / 0.1 / 0 |
| exec / fabber / fighter | 49 / 49 / 49 | 6.0 / 4.6 / 6.8 | 0.5 / 0.5 / 0.9 | 1.1 / 0 / 0 | 9.4 / 9.6 / 11.5 | 2.1 / 1.6 / 2.5 | 447 / 462 / 293 | 18.5 / 0.9 / 0.8 | 0.1 / 0.1 / 0 | 17.5 / 0.6 / 0.7 | 0.2 / 0.1 / 0.1 |
| fixer / foreman / forger | 49 / 49 / 49 | 7.0 / 5.6 / 5.2 | 0.2 / 0.1 / 0.3 | 1.6 / 0 / 3.6 | 9.9 / 9.4 / 10.1 | 1.3 / 1.0 / 1.1 | 448 / 403 / 502 | 2.6 / 1.0 / 3.2 | 0.4 / 0 / 1.1 | 1.7 / 0.8 / 2.1 | 0.3 / 0 / 0.9 |
| gang_leader / gang_member / guard | 34 / 35 / 49 | 8.0 / 5.5 / 9.7 | 1.2 / 0.5 / 1.2 | 3.1 / 4.3 / 0 | 6.9 / 7.3 / 12.8 | 1.4 / 1.5 / 2.6 | 516 / 595 / 161 | 1.1 / 2.2 / 0.8 | 0.2 / 0.1 / 0.2 | 0.4 / 1.6 / 0.2 | 0 / 0 / 0.1 |
| gun / homeless / joytoy | 35 / 49 / 35 | 8.2 / 10.4 / 6.3 | 0.4 / 0.4 / 0.3 | 0 / 0 / 0.6 | 11.5 / 9.6 / 12.0 | 1.9 / 1.8 / 2.2 | 321 / 352 / 318 | 1.0 / 0.9 / 1.3 | 0.2 / 0 / 0 | 0.7 / 0.5 / 1.0 | 0.1 / 0 / 0.1 |
| loader / madam / night_shift | 49 / 48 / 49 | 5.9 / 6.4 / 6.9 | 0.2 / 0.2 / 0.2 | 0 / 2.3 / 1.0 | 8.7 / 11.1 / 8.9 | 1.7 / 2.1 / 1.5 | 429 / 443 / 459 | 0.4 / 5.7 / 1.4 | 0 / 0 / 0.1 | 0.2 / 5.0 / 0.7 | 0 / 0.2 / 0.2 |
| office_worker / orderly / pimp | 35 / 49 / 28 | 4.0 / 6.8 / 10.1 | 0.3 / 0.2 / 1.2 | 0 / 0 / 0.5 | 10.5 / 8.8 / 10.3 | 1.3 / 1.6 / 1.6 | 473 / 441 / 240 | 3.5 / 1.9 / 1.2 | 0.1 / 0.4 / 0 | 3.3 / 0.9 / 0.6 | 0 / 0.1 / 0.1 |
| purist / reporter / ripperdoc | 33 / 49 / 49 | 6.2 / 6.4 / 5.8 | 0.8 / 0.1 / 0.4 | 0 / 0 / 0.2 | 8.9 / 9.4 / 11.7 | 1.7 / 1.4 / 1.6 | 498 / 406 / 334 | 0.4 / 3.2 / 8.2 | 0 / 0.2 / 2.4 | 0.2 / 1.9 / 5.0 | 0.1 / 0 / 2.7 |
| runner / scav / sick | 31 / 35 / 49 | 6.8 / 9.4 / 10.8 | 0.3 / 2.0 / 0.7 | 4.6 / 1.4 / 0 | 9.5 / 6.9 / 9.1 | 1.6 / 2.6 / 1.3 | 690 / 407 / 327 | 2.6 / 0.8 / 1.6 | 0.8 / 0.1 / 0.1 | 1.5 / 0.5 / 1.2 | 0.3 / 0.1 / 0.1 |
| street_worker / super / sweeper | 21 / 49 / 35 | 12.0 / 9.3 / 12.6 | 0.2 / 0.3 / 1.1 | 0 / 0 / 3.4 | 13.0 / 12.9 / 9.4 | 1.5 / 1.8 / 1.4 | 223 / 262 / 560 | 1.3 / 15.6 / 0.7 | 0.3 / 2.5 / 0 | 0.5 / 11.4 / 0.4 | 0.1 / 0 / 0 |
| teacher / vendor / worker / worker_friday | 49 / 35 / 49 / 49 | 3.9 / 11.1 / 7.5 / 6.7 | 0.1 / 0.7 / 0.4 / 0.1 | 0 / 3.7 / 0 / 0 | 9.8 / 10.2 / 9.1 / 13.5 | 1.1 / 1.3 / 1.8 / 2.4 | 459 / 328 / 407 / 327 | 1.1 / 2.3 / 2.3 / 2.5 | 0.2 / 0 / 0 / 0 | 0.8 / 2.3 / 2.0 / 2.1 | 0 |
| **all** | 1994 | 7.1 | 0.4 | 1.1 | 10.2 | 1.7 | 400 | 3.6 | 0.3 | 2.7 | 0.2 |

## The island probe (`isle_m_k4`, 20k scaled, seed 42, 30 days; report only)

The world builds and runs 30 days (mean 207 ticks/s, loaded box: print only). It does not hold 20k: **homeless 7,823 at d0** (the gen city: 800), 6,133 at d20, **8,032 at d29**; employed 4,420 (0.22) → 5,697 (0.29); starving (`d1_starving`) 0 until d10, 20 at d20, 150 at d25, 231 at d29; 1 starvation death; 105 violent deaths (+15 off screen); 21 Murders; events: 6,032 Housed, 4,923 Hires, 2,688 Evicted, 316 Founded. The d30 list against the gen city's d30 (seed 43): homeless 8,032 / 258, sick 5,186 / 157 (malnutrition), worker 5,221 / 12,482, cook 45 / 437, ceo 34 / 102, gang members 246 / 589; no dealer, teacher, madam, office worker. `IMPORT_GRAPHICS.md` already reads k = 4 as too coarse for 20,000 (93 Blocks per 1,000 against the generator's 200): this is that, at run time.

## The devices

- `shadow --assert`: **133 PASS, 0 FAIL** (70 s).
- `shadow --lives --population 20000` (345 s), the `all` row against `LIVES_AT_WORK.md` § 13's closing 20k row (`e566e29`): n 222 → 220, idle 3.59 → 3.95 h/d, named/wk 1.05 → 1.01, empty HangOuts 0.39 → 0.42, Served-able 0.30 → 0.32, met 0.44 → 0.42, enc/shift 3.08 → 2.69 (coloc 10.01 h/wk, stat-spouse 0.10; travel 37.9 h/wk). The tree is `a25fdc5` (8c fix round + the close) against 8c's; the tool change moves no `--lives` column (the Meeting was `other`, which none reads).
