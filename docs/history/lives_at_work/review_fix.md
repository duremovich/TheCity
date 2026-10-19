# Code-review fix round: Lives at work (23a161d..f795548)

Branch of worktree agent-a75c6b429837e517e, base f795548. **Final sha 1691260.** Not merged.
Every finding was confirmed with a test that fails on the base before the fix (doctrine 9), except 8, 9 and 10, which change no behaviour and were proved byte-identical.

## Findings

| # | verdict | commit | note |
|---|---|---|---|
| 1 owner_operator skips the gate | **fixed** | 52e216a | Now reads `careers::hire_band(role)` + `admits`, `life::quit_blocks`, `dollhouse::hire_ok`. Test `household::test_owner_operator_respects_the_hiring_gate` (the retired owner was rehired on base). |
| 2 stale CabRide | **fixed** | ce80f2e | `ride_of` only holds a ride while the driver `can_drive` (living, free, fare-duty Job on shift). `streetwork::hourly` drops the stale ones; the rider's leg was already timed at booking, so he still arrives, as with the off-screen cab. `leisure::spot_of` and the stand-by Rest read the fare only inside the driver's Work plan (`streetwork::driving`), so a goal that pre-empts it reads its own spot. Test `street::test_a_stale_cab_ride_is_dropped`. |
| 3 first-hour copy-paste | **fixed** | 7f5b97e | `Job::first_hour(tod)` reads the wrap the same way `shift_key_at` and `shift_end` do. All 4 sites call it. Test `service::test_a_wrapped_night_shift_has_one_first_hour` (base: the madam counted the night worker in again at 00:00). |
| 4 retire to kin leaves spouse | **fixed** | 9525ef1 | Chose to move the couple. A co-resident living spouse moves too (`ages::move_in_with`), so this needs room for two; with room for one he does not retire to kin. Written into the LIVES_AT_WORK phase 6 Retirement bullet. Test `careers::test_retiring_to_kin_moves_the_couple`. |
| 5 jail capacity holes | **fixed** | 3685d92 | `jail_suspect` re-checks `has_cell`; a full Station books to the Precinct. New `stations::precinct_bound` counts the Precinct's prisoners plus Station prisoners whose term runs past the next van. Every booking path reads it (escort, the Statistical vagrant in `street.rs`, riot, robots, `player_arrest`). `van` fills only to capacity; the rest wait for the next van. With no Station standing it equals the old count. Tests `stations::test_a_full_station_never_books_past_its_cells` and `test_station_prisoners_bound_for_the_precinct_count_and_the_van_respects_it`. |
| 6 remember_empty tile | **fixed** | c584d46 | At home it stores the Home's outside door. Test `household::test_an_empty_home_hangout_is_remembered_at_the_spot`. |
| 7 fixer room overflow | **fixed** | 27ad209 | `fixer_book` only takes staff with room this hour. When all are full it falls back to the owner (if he holds no post there), else nobody: the contract stands but the encounter is not credited, per the spec's "credited world-side at that hour". `fixer_meet` uses `saturating_add`. Test `tower::test_the_fixers_book_respects_the_hours_cap`. |
| 8 literals → config | **fixed** | 22163a1 | Two new tables. `[household.visit_urgency]`: the 5 urgencies and the curve. `[service.tune]`: the bouncer, notice and jailbreak valences, the camp beating grief (salience and valence), the fare-dodge grudge, the break-out thresholds and the pimp's two GangWork curves. Both are `skip_serializing_if` at their defaults (= the shipped values). Seed 42 × 30 against the fix-7 binary (27ad209): events, CSV and save **IDENTICAL**. I compared to the fix-7 binary, not main's, because fixes 1-7 move the city on purpose. |
| 9 dead code | **fixed** (partly not a bug) | 621bfc2 | Deleted `business_of` and `waits_for_business`; their test now reads `business_holders`. Dropped `taught`'s unused `school` parameter. `owner_operator`'s `role` is used since fix 1. **`stims::stat_buy_one` kept, not a bug:** it is `tests/stims.rs`'s hook into `stat_buy`. Its doc said "not used"; that is corrected. |
| 10 full_cab clone / exec_ride scan | **fixed** | 9f6e36e | `full_cab` borrows the Job for the cheap checks and clones only for the roll. `exec_ride` first checks that the target is a corp building (`corp_of_building`) whose exec he is, and only then runs the `exec_corp` scan. `Corp.buildings` and `Building.owner` are kept in step by `transfer_building`. Fixes 9 and 10 against 6dc06d6, seed 42 × 30: **IDENTICAL**. |
| 11a newborn keep, widowed parent | **fixed** | 6dc06d6 | `household::daily` now reads every Household with a newborn, one charge per Home. Test `household::test_a_widowed_parents_newborn_is_kept_too`. |
| 11b profession days on day 0 | **fixed** | 6dc06d6 | The once-a-day guard now holds on day 0 too. Test `careers::test_day_zero_counts_one_profession_day` (base: 2). |

The moved city broke one setup: `person::probe_pimp_purse_grows_on_a_pitch_night` found no gang led by a veteran on day 12. The setup now waits day by day, up to day 20, for the first evening with one. No assert changed (4481b57).

No save shape change, so SAVE_VERSION stays 45 (fixes 2 and 7 change values only).

## Counters: `phase_measure --base f795548 --skip off --arch elder,cabbie,exec,madam`

`retiree` is not an archetype, so I used `elder`.

| counter | seed 42 | seed 43 | seed 44 | mean |
|---|---|---|---|---|
| Murders | 54 -> 101 | 88 -> 116 | 101 -> 79 | 81 -> 99 (+22%) |
| J24 employed | 0.795 -> 0.797 | 0.758 -> 0.793 | 0.766 -> 0.750 | 0.773 -> 0.780 |
| J25 homeless | 0.010 -> 0.014 | 0.013 -> 0.011 | 0.014 -> 0.016 | 0.012 -> 0.014 |
| arrests | 2,593 -> 2,489 | 2,455 -> 2,427 | 2,329 -> 2,421 | 2,459 -> 2,446 |
| wallets (last day) | 126,144 -> 93,609 | 73,041 -> 73,025 | 80,241 -> 71,679 | 93,142 -> 79,438 (-15%) |

| device | n | idle h/d | shifts | Served-able | met | enc/shift |
|---|---|---|---|---|---|---|
| elder | 90 -> 90 | 6.11 -> 5.44 | 216 -> 272 | 0.12 -> 0.15 | 0.27 -> 0.39 | 1.41 -> 2.15 |
| cabbie | 18 -> 16 | 3.77 -> 4.62 | 81 -> 78 | 0.14 -> 0.10 | 0.26 -> 0.31 | 0.26 -> 0.32 |
| exec | 31 -> 29 | 6.41 -> 6.65 | 148 -> 133 | 0.18 -> 0.12 | 0.80 -> 0.92 | 29 -> 31 |
| madam | 3 -> 3 | 5.07 -> 4.23 | 12 -> 9 | 0.92 -> 0.89 | 0.83 -> 0.44 | 4.67 -> 2.89 |
| all | 1,820 -> 1,822 | 4.48 -> 4.47 | 8,434 -> 8,745 | 0.31 -> 0.29 | 0.43 -> 0.44 | 3.73 -> 3.72 |

**Why things moved.** The first event that differs is on day 0: fix 4 moves a retiree's spouse in with him.
- **Retirements to family** went 291/243/260 -> 266/248/204, because the child's Home now needs room for two.
- **Retirements on savings** went 16/0/25 -> 2/2/1, and **frail exits** 28/75/65 -> 29/52/33. That is fix 1: on base, a retired or frail owner was rehired into his own business every midnight and then retired or let go again.
- **The madam's enc/shift** fell 4.67 -> 2.89. That is fix 3: the 00:00 double count is gone.
- **Murders** went up on seeds 42 and 43 and down on seed 44. That matches the chaos already recorded in the phase 6 FINDING.
- **Wallets** fell mostly on seed 42, which was already the base's high seed. I haven't traced this to a particular fix (30-minute cap). It is logged as **OPEN-134** (finding, open).

Nothing was retuned.

## Change tier (`python tools/test.py change`, once, on 1691260's tree minus the OPEN row)

    [change] GREEN; BUDGET OK 3m38s of 4m00s

`test.py lint`: 0 problems, 6 warnings, all file length. Logs: `target\test-logs\change-20261019-182846`, `target\measure\review`, `target\measure\fix8`, `target\measure\fix10`.

## Merge-set follow-up: god_kill_all_teachers (3 of 4 held on day 30)

**Not a gate bug, and fixes 1 and 4 are not involved.** I traced seed 42 with a throwaway diag test, deleted afterwards:
- All 4 posts refilled at day 20's midnight (Priya Crane, Hale Ito, Amara Santoro, Kuro Mbeki).
- Hale Ito is a gang member who brawled and extorted through days 20-29. A fight on day 29 took him out of the post.
- His post refilled at day 30's midnight (Nova Jarrow).

The assert read the holders once, at the start of day 30, and caught that one-day gap. The setup now takes the most posts held by living teachers on any one evening of days 21-30, and still requires `held >= killed`. The count is not loosened.

Commit **ad6e1a0** (test only). **New final sha: ad6e1a0.**

- `python tools/test.py merge --only god`: `[merge] GREEN; BUDGET OK 52s of 8m00s`. 16 passed. The run reads "posts: 4 emptied on day 20, 4 held …".
- `python tools/test.py change`: `[change] GREEN; BUDGET OK 3m32s of 4m00s`.
