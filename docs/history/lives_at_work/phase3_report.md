# Phase 3 report: the civic counterparts and the round

- Worktree `D:\TheCity\.claude\worktrees\agent-aeb9f5f4cf9b9e4ee`, branch `worktree-agent-aeb9f5f4cf9b9e4ee`, ff-merged to main `899acc2` (confirmed: a HANDOFF note on top of the phase 2 merge `596cd8c`). One squashed commit on top of `899acc2`; never merged.
- Main's binary for the base runs, the off identity and the A/B: built from `git archive 899acc2` into `scratchpad/p3/main_src` (its own target) and **run from that root** (its `assets/config.toml` lacks the phase 3 keys; a first base batch run from the worktree root produced empty outputs and a false "DIFFER", the identity then checked against the phase 2 tree's events as a stopgap and finally against this binary).
- Final sha: `e763ee2` (one squashed commit on top of `899acc2`, 27 files, the god pick fix folded in; the tree is clean after it; `698beba` is gone).

## Footers (final tree)

```
[change] GREEN; BUDGET OK 3m51s of 4m00s
[merge --only god] GREEN; BUDGET OK 1m05s of 8m00s (17 passed)
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 49
```

The six lint warnings are the standing over-1,500-line files. `cargo fmt --check` clean; `git ls-files --eol | grep -c w/crlf` = 0. (One run before read `BUDGET BREACHED 4m27s`, the next two 3m52s and 3m48s: box noise, not the phase's tests; `civic` runs 1.8 s, `person` grew 12 → 15 s with the two probes.) After the last run only a comment in `tests/core.rs` changed (the door-payment bullet counts days with a payment, [103, 103, 103], not coins). The six new bullets printed: School day taught [31, 272, 46], Camp headcount [85, 92, 88], super's door payment [103, 103, 103] days, arrears notice [1331, 1228, 1295], parent's Camp visit [123, 97, 90], neglecting parent-day [190, 202, 215].

## Built (docs/LIVES_AT_WORK.md § 14 "Phase 3" has the deviations in full)

- Config `[service]`: `civic = true` (`CITYSIM_CIVIC_OFF=1`), `p_camp_beating 0.02`, `p_camp_escape 0.004`, `buyout_fee 60`, `buyout_food_days 7`, `breakout_crew 3`, `neglect_days 14`, `teacher_floor 0.5`; four `[[serves]]` rows (`teacher` pupil, `camp_warden` inmate, `jailer` inmate, `super` tenant with `event_on = ["notice"]`), each `civic = true`; `[[trades]] super`: `round_near = true`, `tied_housing = true`. `Config::strip_civic` drops all of it whenever the switch is off by any path (the toml, the env), so the off save matches main byte for byte; a test that flips the switch after the load calls it itself.
- State: `Child.edu_taught` (skip at 0; SAVE_VERSION 37), `VisitWhy::Camp(CampAnswer)` on `PendingVisit`, `World::notices_today` (saved while it holds anything, cleared at midnight: the mid-day save identity test caught the unsaved first version), `EventKind::{CampEscape, CampBuyout, CampBreakout, Notice}` (ALL 219), `Flow::Buyout`, `ServeOutcome::Notice`, `ReleaseWhy::{Escaped, BoughtOut}`, 13 `Jobs2Cols` columns (`camp_beatings, camp_escapes, camp_headcounts, camp_visits, camp_buyouts, camp_breakouts, camp_neglect, notices, door_rent, super_stops, tied_housed, flow_buyout, taught_days`; cli header +13).
- `service.rs`: `civic_on`, `civic_hour` (the teacher's roster and the warden's headcount at the shift's first hour), `taught` (the taught day and the parent at the door), `tenant` (notice, door rent), `super_stop`, `jailers_passed`. `camp.rs`: `warden_rolls` (beating, escape), `send_home`. `household.rs`: `camp_answers_due`, `answer_of`, `camp_answer` (visit / buy-out / break-out), `tied_housing`. `trades.rs`: `new_round` near, `on_round`, `round_for`, the stop in `round_plan`. `lod.rs`: the Statistical super's stops at the shift's first hour. `law.rs`: the jailer's meal. `raid.rs`: the jailer named. `ages.rs`: `on_mature`'s gate. `ownership.rs`: `tied_here`, `collect_rent`/`settle_rent` free the tied, `settle_rent` pub(crate). `life.rs`: the relocation pass leaves a landlord-housed household. `shadow.rs`: a round leg reads as work.
- Tests: `tests/civic.rs` (10: the taught day and no teacher no day; the near round; tied move-in with and without room and no rent; the stop's notice and door rent; the jailer's meal; the buy-out and its refund; the break-out and the Abduction; the escape and beating rolls; the answer chosen from the household and the neglect tally; the off city), `tests/person.rs` (2 probes), `tests/core.rs` (6 printed bullets), `tests/lod.rs` (the door rent share row, printed).

## Statistical mirror (§ 6)

The teacher's and the warden's rosters, the jailer's meal and the warden's rolls are world-side (a Statistical server is credited by `present`); the super's door rent moves at the shift's first hour for both tiers (`lod::stat_work` runs the same stop list when `shift_attended`); the camp answers are chosen by the same midnight roll, a Statistical parent's resolved at the hour (`household::hourly`); tied housing and the near round are midnight and shift-start rules for every tier. No stat-table regen (D14).

## Off identity (CITYSIM_CIVIC_OFF=1 against main's binary, 42-44 x 30)

Saves 12,777,695 / 13,030,908 / 12,920,091 B on both sides, `cmp -l` one differing byte per seed (`save_version:36` → `37`); events byte-identical on 42, 43, 44; the report CSV's 964 shared columns identical on 30 rows per seed (the 13 new columns the only additions).

## Numbers against the base (2k 42-47 x 120, days 30+, mean a day; main → phase 3)

| column | main | phase 3 | per seed totals (main → phase 3) |
|---|---|---|---|
| taught_days (= served_pupil) | — | 1.09 | 31 272 46 45 53 177 |
| school_days | 1.17 | 1.10 | 72→31 86→273 171→46 67→45 177→53 89→177 |
| camp_headcounts | — | 0.86 | 85 92 88 93 97 95 |
| served_inmate (the Jail's meals) | — | 59.7 | 6114 5953 6195 6176 5695 5964 |
| served_tenant / super_stops / notices | — | 790 / 117 / 10.5 | notices 1331 1228 1295 1074 1266 1018 |
| door_rent (coins) | — | 143 (21 % of rent paid) | 18541 18002 18355 17273 18237 17634 |
| rent_paid / rent_short | 676 / 58.7 | 676 / 53.3 (−9 %) | |
| evictions | 2.64 | 2.45 (−7 %) | 319→258 238→253 261→235 305→246 254→255 257→240 |
| homeless | 41.0 | 35.4 (−14 %) | 5339→4311 4071→4083 4906→3920 5390→3652 4159→4371 3581→3627 |
| tied_housed | — | 0.196 | 28 31 27 31 25 30 |
| super_rounds | 2.43 | 2.85 (+17 %) | |
| children_taken | 0.085 | 0.135 (+59 %) | 8→14 14→20 12→20 6→13 11→9 11→14 |
| camp_children | 6.25 | 5.48 | |
| camp_beatings / camp_escapes | — | 0.148 / 0.030 | 14 8 19 16 9 16 / 2 4 5 3 1 2 |
| camp_visits / camp_buyouts / camp_breakouts | — | 1.09 / 0.002 / 0.043 | 123 97 90 128 68 115 / 0 0 0 1 0 0 / 1 8 6 2 3 4 |
| camp_neglect (parent-days) | — | 1.84 | |
| flow_buyout | — | 60 (seed 45, over the run) | |
| served_waited | 212 | 223 (+5 %) | |
| deaths_violence / thefts / arrests | 1.60 / 2.84 / 24.4 | 1.58 / 2.93 / 24.2 | |
| employed_share / hangouts / housed | 0.773 / 361 / 2.52 | 0.775 / 349 / 2.38 | |
| wallets | 46,190 | 44,910 (−2.8 %) | |

- Murders (`analyze_run.py`, 120 d, per seed 42-47): main 74, 115, 132, 132, 29, 196 (mean 113); phase 3 55, 117, 225, 138, 64, 97 (mean 116). J24 / J25 per seed, main → phase 3: 42 0.759/0.025 → 0.768/0.021; 43 0.777/0.019 → 0.774/0.019; 44 0.758/0.023 → 0.767/0.018; 45 0.764/0.025 → 0.793/0.016; 46 0.785/0.018 → 0.762/0.020; 47 0.795/0.016 → 0.781/0.016.
- Save size on, 30 d seed 42: 12,945,486 B against main's 12,777,695 (+1.3 %).

## Lives probe (shadow --lives 2k, 42-44 pooled, day 19, 7 d; main → phase 3)

| arch | n | idle h/d | shifts | Served-able | met | enc/shift |
|---|---|---|---|---|---|---|
| teacher | 12 | 4.26 → 3.84 | 68 → 65 | 0.00 → 0.03 | 0.00 → 0.45 | 0.00 → 0.60 |
| camp_warden | 6 | 4.23 → 3.51 | 35 → 33 | 0.03 → 0.09 | 0.00 → 0.79 | 0.00 → 1.36 |
| super | 88 | 3.67 → 4.40 | 523 → 516 | 0.22 → 0.26 | 0.00 → 0.97 | 0.00 → 27.8 |
| guard | 90 | 4.10 → 4.15 | 534 → 522 | 0.72 → 0.73 | 0.01 → 0.14 | 0.01 → 0.17 |
| all | 1806 | 4.12 → 4.24 | 8933 → 8492 | 0.33 → 0.29 | 0.07 → 0.32 | 0.15 → 2.22 |

No `jailer` archetype exists in the probe or in `shadow --pick` (the brief's `--pick ... jailer` was skipped by the tool); the Jail's row shows through the guards' `met`. The full rows are § 13's "after phase 3" table.

## Throughput

Alternating A/B, seed 42 × 30 d, one run at a time, main from its own root: main 4,317 / 5,268 / 5,644; phase 3 6,097 / 6,433 / 6,295. The first main round reads as a cold start; no regression readable on this box (CLAUDE.md: print, never tune). The batch runs' `ticks_per_sec` (six in parallel) read 3,375 → 3,294 (−2.4 %).

## Diaries (`shadow --pick super,teacher,camp_warden --count 2 --days 7 --start-day 19`, `scratchpad/p3/diaries/`; main's supers in `diaries_main/`)

- **A super's travel, before and after** (hours a day over the week, from the day summaries): main's Tomas Hallow 8.3 / 13.3 / 10.4 / 6.7 / 7.2 / 8.9 / 7.2 (work 3.9-4.2), Juno Adeyemi 12.0 / 4.7 / 6.9 / 9.4 / 7.0 / 10.7 / 11.9 (work 1.0-2.8): V4's 63-79 h a week. Phase 3: Zelda Reyes 3.3 / 3.1 / 3.3 / 5.3 / 3.8 / 2.9 / 4.5 (work 9.8-10.6), the second super 1.3 / 7.7 / 0 / 0 / 0 / 3.5 / 3.5 (work 8.1-9.4); both housed in the landlord's Block (Zelda at Block#196, her post Block#181 in the same district; the other at his own post, Block#211). The probe: 165 of 540 shift minutes walking against main's 311.
- **A super collecting and serving a notice**: `d23 11:17 Notice: Zelda Reyes served notice on Cutter Ito at Block#196` (Cutter is her husband, a Bartender behind on his share; she lives there rent-free as the landlord's super); `10627 Notice: Ysolde Duarte served notice on Marek Vance at Block#97`; the door rent is the `[money]` Rent lines at the stops (143 coins a day city-wide, 21 % of the rent).
- **A teacher with named pupils**: Rafe Park (teacher #1090, School) `d19 09:48 [memory] served Slate Moreau#1008`, and again on d21, d22, d23, d24, d25: the 2k School's one pupil, a School day each weekday. (The second teacher's School had no pupil that week.)
- **A warden's count and an escape**: Kuro Singh (camp warden #2392, Work Camp#520) `d19 09:35 served Juno Delacroix#2015`, `09:54 served Tomas Crane#834`, `12:39 served Ghost Reyes#1526` (a late arrival), the same names on d21; `d22 09:00 served Raven Achebe#2900 (valence −0.10)`. The escapes in the 120-day events: `86400 CampEscape: Lavinia Reyes slipped out of Work Camp#520 to Block#20` and again at 135360 (taken back in between).
- **A parent's visit, a break-out**: `35340 Visited: Idris Nakamura visited Aldo Nakamura at Work Camp#520 (a Camp child)` and again at 42900 (the weekly visit); `127800 CampBreakout: Ottilie Lindqvist broke Kieran Lindqvist out of Work Camp#520, to Block#104`; seed 43: `Imogen Wozniak broke Xia Mbeki out of Work Camp#520` twice (29700, 68340: taken back between). The one buy-out (seed 45) moved 60 coins as `Flow::Buyout`.

## The fix round inside the phase (what the first measured build showed)

0. **The day's notices were unsaved** and `tests/save.rs::test_word_save_mid_day_bit_identical` caught it (a resume at 11:40 served the morning's notices again): saved while non-empty, cleared at midnight.

1. **Tied housing bounced households every midnight** (`tied_housed` 126 on seed 42 alone; one super 50 moves in 120 days). Two causes, traced with a diag: (a) the relocation pass moved the super's spouse nearer *her* work and the super with her, `tied_housing` moved them back; (b) two supers of two landlords married each other, each landlord's rule moving the couple to its Block. Fixes: `life::relocate` leaves a household in which anyone is `tied_here`; `tied_housing` leaves a worker whose co-resident is housed by that co-resident's landlord. Now 28-31 move-ins a run (mostly day 0), the most-moved super 3.
2. **A returning child needed room** and most pairs live in a flat of two: every bought-out, broken-out or escaped child went to the street. `camp::send_home` houses the child in a parent's standing Home without the capacity check (as `spawn_child` does). Written as a deviation.
3. **A Camp headcount counted an empty Camp**: counted only with children.
4. **Main's binary ran from the worktree root** and died on the new config keys; the base batch re-run from `main_src`.

## Test changes (flagged)

1. `tests/camp.rs::test_release_at_adulthood_with_camp_raised`: runs with `Config::strip_civic()` (on, a parent's weekly answer took the child home in the four days before its coming of age).
2. `tests/person.rs::probe_married_pick_is_home_by_nine_with_the_spouse` (phase 2's), `probe_far_worker_with_one_rent_moves_near_work`, `probe_worker_makes_one_grocery_run_to_the_nearest_detour_market`: pinned to `strip_civic()`. Tied housing moves the seeded supers' households on day 0 and the city diverges from there; the picks on the moved city are a Wanderer at 21:00 (pick 600, 7 evenings with the spouse that week but out at 21:00 on the probe's night), a far Farmer with no single trade partner within reach, and a worker with two grocery runs in the window; none touches a civic mechanism.
3. `tests/god.rs::god_flood_guns` (the merge tier, found by the orchestrator's merge set on `698beba`): the scenario floods "the weakest gang on day 20" and asserts the crate is drawn by day 60. On the civic city (tied housing moves the seeded households on day 0; the gangs recruit differently from there) the weakest gang with a Hideout on day 20 was The Drowned Choir with three members (main's weakest: The Unplugged, four members, two free), and three is under a muster's `raid_min_members` with a leader who stays back: no raid, no issue, `drawn` 0 against 200 staged (the story: weekly collections "0 to 0 members", the leader jailed on day 58). With `CITYSIM_CIVIC_OFF=1` the scenario passes as on main (The Unplugged, one draw). No phase 3 mechanism touches the issue path (no break-out crew borrows a muster, the Hideout stands, nobody is housed away by tied housing). Fix: `flood_pick` takes the weakest gang with a Hideout **and a crew to march** (`raid_min_members + 1` free living members: the marchers plus the leader), the lawful-Farmer shape; the 200 and `drawn > 0` asserts stand. On the civic city it picks The Unplugged (eight members, four free): 200 placed, 3 draws, armed fights 146 against the control's 156, armoury issues 16 against 6. `test.py merge --only god` GREEN after it.
4. `citysim-cli/tests/cli.rs`: the header string and `+ 13` columns.
5. New: `tests/civic.rs` (10), `tests/person.rs` two probes (`probe_parent_with_the_fee_buys_the_child_out_within_fourteen_days`: the household holds 80 coins past the midnight rent and a stocked pantry at each roll, topped up at each eve, since the city takes 30 of 80 in a day and a jobless parent's arrears the rest; the first pairs are tried until a parent lives the fortnight, seed 42's first pick being abducted, harvested and killed on day 13; the child's home is read against the parent's, the family may move; `probe_super_walks_under_four_hours_a_shift`: day 22, a weekday, 165 of 540 minutes against main's 311), `tests/core.rs` six printed bullets, `tests/lod.rs` the door rent share row.

## Deviations (§ 14 phase 3 block, in brief)

`Inmate` pooled for the Camp and the Jail; the civic rows and keys stripped off; `edu_taught` beside `edu_days`; no `repairs_per_stop` (no Block condition); the super serves his own round (`arrive` resolves among the place's staff, his employer is one Block); the near round's walk unchanged after the pick; tied housing frees the rent in `collect_rent`/`settle_rent` and moves in after `reunite`; the jailer's meal is an arrival, the breakout names the first jailer; the warden's rolls in `camp::daily`; a returning child needs no room; the parent's answer on a hashed day of the week, the hurt-kin visit outranking it; the visit not queued at the warden's counter, `last_met` and belonging written by the arm (no `interacted`: a child has no edge table); the break-out's arrest by the strongest warden standing (`law::arrest` takes any arrester; the cuffed parent waits for the Law) or the filed warrant for a Statistical parent, the Abduction filed on a win too; the short-paid buy-out refunded; no regulars on the civic rows; the super's tests in `tests/civic.rs`.

## Open items for the orchestrator

- The Camp arms are rare at 2k (62-90 takes in six runs; one buy-out): unit-tested and printed, never bullets; the 20k city was not run this phase (no time asked for it).
- No `jailer` archetype in the lives probe / shadow picks: the brief's device for the jailer is unmeasured beyond `served_inmate` (~60 meals a day) and the guards' `met`.
- `children_taken` +59 %: escaped and broken-out children are taken again (seed 42's Lavinia Reyes twice); printed, a feature of the answer, not retuned.
- A super serves notice on his own spouse when she is behind on her share while he lives rent-free (Zelda Reyes on Cutter Ito): the tied rule frees the worker, not the household; a design question for the gate.
- The parity test was not run this phase (the milestone tier; phase 2's rows stand; the door rent share row is new and printed).
- `scratchpad/p3/` holds `main_src` (main's source and binary, ~10 GB target), the base/on/off runs, the diaries and the scripts; the worktree's `target/` is for the orchestrator to delete at merge.
