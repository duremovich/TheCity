# Phase 2 report: the evening and the household (with the fix round)

- Worktree `D:\TheCity\.claude\worktrees\agent-aeb9f5f4cf9b9e4ee`, branch `worktree-agent-aeb9f5f4cf9b9e4ee`, ff-merged to main `70e9589` (confirmed; a HANDOFF note on top of the phase 1 merge `593e8cf`; the sim is `ed26f53`'s). One squashed commit on top of `70e9589` (the fix round folded in); never merged.
- Main's binary for the A/B and the off identity: the worktree's `target/release/citysim-cli.exe` built at `70e9589`, copied to `scratchpad/p2/base/citysim-cli-main.exe` before any edit; the base runs (42-47 x 120, 42-44 x 30 saves/report/events, `shadow --lives` 2k, 20k 42 x 30) ran on it first.
- Final sha: `17c2751` (one squashed commit on top of `70e9589`, the fix round folded in; the tree is clean after it).

## Footers (final tree)

```
[change] GREEN; BUDGET OK 3m50s of 4m00s
lint: 0 problem(s), 6 warning(s); god 20/20, asserted bullets 60/60, ignores 49
```

The lint warnings are the standing over-1,500-line files (god, hitmarket, person, wages, shadow.rs). `cargo fmt --check` clean; `git ls-files --eol | grep -c w/crlf` = 0.

## The fix round (the coordinator's one fix before the merge)

The first measured build offered the own Home unconditionally ("at any distance"), so it won the Free rung whenever nothing else was near, 06:22 before a shift included: never-named 1,016 → 988, empty 0.27 → 0.30, idle 4.05 → 4.54, and most of its 180 home HangOuts a day were 21-minute checks of an empty flat. The fix (`household::company_due`): the own Home is offered only with company to be had there, i.e. `home_term > 0` (a household body inside or a child at home) or a resident **with a moving body (Full or Coarse)** whose `hunt::habit` at `now + 60` is the Home. No new knob.

Why the clause is restricted to bodied residents (the one deviation from the brief's wording, measured): `hunt::habit` says "home" from 20:00 for anyone who slept at home 7 of 14 nights, and a Statistical resident's `Position` is a demotion snapshot that never arrives. The diag (seed 42, day 19, every adult whose shared Home had nobody inside):

| hour | adults with an empty shared Home | due by habit, any resident | a body inside an hour later | due by habit, bodied residents only | a body inside an hour later |
|---|---|---|---|---|---|
| 09 | 1,752 | 1,221 | 5 | 92 | 35 |
| 12 | 1,762 | 1,231 | 5 | 63 | 1 |
| 17 | 1,754 | 1,394 | 13 | 145 | 2 |
| 18 | 1,749 | 1,386 | 22 | 158 | 23 |
| 19 | 1,733 | 1,702 | 35 | 317 | 39 |
| 21 | 1,733 | 1,706 | 29 | 264 | 29 |
| 23 | 1,694 | 1,667 | 17 | 187 | 9 |

With any resident's habit the clause offered the Home to nearly everyone from 19:00 and the body never came (the first fix build: 48 own-Home empty leaves a day, 31 % of the home HangOuts). Restricted to bodied residents it offers a few hundred and is right one time in ten; what is left is 22.6 own-Home empty leaves a day at 2k (18 % of 124 home HangOuts; the diaries: 5 empty of 29 at a Home). Whether a Statistical spouse should count as *home by the clock* (the mirror: a Statistical server is credited by the clock in phase 1) is the open design question for the gate: with it, the Full tier's home evening would see the ~95 % of households whose other members are Statistical; without it, the device can only move for couples with two moving bodies. New column `home_empty_leaves` (Jobs2Cols, skip-zero) carries the number.

## Built

- Config `[household]` (`HouseholdCfg`; `CITYSIM_HOUSEHOLD_OFF=1`, also cleared by `CITYSIM_LIVES_OFF=1`): `enabled, home_w 0.5, child_w 0.3, visit_hosts 3, home_dice false, empty_leave_ticks 20, empty_penalty 0.3, empty_memory_hours 6, newborn_days 30, newborn_away_mult 0.3, newborn_energy_mult 1.15, newborn_cost_per_day 0.5, night_from 21, night_to 6, abandon_days 7, elder_visit_days 7, visit_hour_from 10, visit_hour_to 20, visit_flat 0.2, visit_belonging 0.2`. The deed tables gained `employed`, `laid_off`, `born` (reach0/deed_sal 0.2-0.3, deed_sev 0), left out of a save at 0 and zeroed by the off switches.
- State: `SpotKind::Home(EntityId)`, `GoalKind::Visit` (GOAL_ORDER 32, after Rescue), `EventKind::Visited` (ALL 215), `Deed::{Employed, LaidOff, Born}` (ALL 22), `Brain.empty_spots` (SmallVec 4, skip if empty), `Household.{newborn_until, newborn_carer}` (skip if unset), `World::visits` (saved while pending), SAVE_VERSION 36, 9 `Jobs2Cols` columns (`home_hangouts, friend_hangouts, home_with_spouse, empty_leaves, home_empty_leaves, reunited, kin_visits, elder_visits, carers`; left out of a save at 0; the cli header test +9).
- `systems/household.rs` (new): `home_spots`, `company_due`, `home_term`, `home_partner`, `hanging_at_home`, `residents_inside`/`any_inside`, `empty_leave`, `remember_empty`, `company_only`, `company_at`, `empty_mult`, `reunite`, `on_birth`, `new_parent`, `carer`, `energy_mult`, `damped`, `away_mult`, `business_of`, `business_holders`, `waits_for_business`, `owner_operator`, `is_member_of_owner`, `daily`, `visits_due`, `visit_now`, `considerations`, `plan`, `can_start`, `visit_done`, `hourly`. Hashed purposes `0x4855_000n`; no world-stream draw; money untouched (the newborn's keep is pantry stock).
- Hooks: `leisure::spots_near_with` (the Home spots), `best_spot_kind` (the household term, the empty-spot mult, the carer's away mult, the company-only filter), `choice`/`hangout_plan` (a Home pick carries the Home as `venue`, walks `GoTo(Home)`/`GoTo(TargetHome)`, runs inside; `can_start(HangOut)` allows it; no Seat), `hangout_done` (the household present as company, the home counters, the empty-spot memory, the own-Home empty count), `actions::finishes_early(HangOut)`, `actions::on_complete(Rest)` (a Visit's end), `needs::run`, `utility::think_with_offer` (the carer's damping), the scripted Visit (`goap::goal_state`/`plan::plan_for`/`exec::start_step`), `ownership::daily` (`reunite` before `rehouse`), `living::run` (`household::hourly`, `household::daily`), `demography::spawn_child`/`hire`, `economy::dismiss_as`, `demography::job_search_pass_with` (`owner_operator` first), `HirePool::pick`/`pick_candidate` (the holders read once a pass), `wages::past_cap_pass`/`newest_hire`. The lives probe counts the household present as a home HangOut's company.
- Tests: `tests/household.rs` (10: + `test_own_home_is_not_offered_empty`, an adult living alone has no Home spot while the married pick with the spouse in has it), `tests/person.rs` (2 probes), `tests/core.rs` (8 printed bullets).
- Docs: LIVES_AT_WORK § 14 phase 2 block (deviations incl. the gated Home, the measured table, the lives reading with the fix round, the test changes, the parity rows) and § 13 after-rows (the fix tree's 2k lives table); ARCHITECTURE §§ 5, 6, 7.

## Statistical mirror (§ 6)

Reunite, the newborn flags, the business scan and `visits_due` are midnight rules for every tier; a Statistical visitor is credited at its hashed hour; the deeds post for every tier. The home evening moves the Full tier's HangOut, venue and meal rates; the Statistical table learns them at the gate's one regen (D14), which is why the parity rows below are out. Whether a Statistical spouse is "home by the clock" for the Full tier's evening is the open mirror question (above).

## Off identity

The off path is untouched by the fix round (`company_due` sits behind `home_spots`'s `on()` gate), so one seed was re-checked as the coordinator allowed: phase 2 binary with `CITYSIM_HOUSEHOLD_OFF=1` against main's binary, seed 42 x 30 d: save 12,398,418 B both sides, one differing byte (`save_version:35` → `36`); events byte-identical; 955 shared CSV columns, 0 differing cells (the 9 new columns the only additions). The first build had the same on 42, 43 and 44.

## Numbers against the base (2k, seeds 42-47 x 120, days 30+, mean a day; main → first build → the fix)

| column | main | first build | fix | per seed (fix totals) |
|---|---|---|---|---|
| hangouts | 355 | 369 | 361 (+1.5 %) | 46945 43126 43864 45547 43831 41717 |
| hangout_contacts_mean | 1.10 | 1.21 | 1.27 | |
| home_hangouts | — | 180 | 124 | 17740 16499 17957 16878 17178 15821 |
| friend_hangouts | — | 42.9 | 43.8 | 5029 3567 4178 4276 4347 4381 |
| home_with_spouse | — | 29.1 | 24.9 | 4227 4068 4447 4570 4398 3869 |
| empty_leaves | — | 140 | 127 | 15517 14109 14426 15109 14002 13980 |
| home_empty_leaves | — | (≈ 140 by the diaries' share) | 22.6 | 2407 2487 3141 2738 2623 2464 |
| reunited | — | 0.104 | 0.115 | 21 16 19 17 11 14 |
| kin_visits | — | 6.96 | 7.29 | 727 659 698 749 728 786 |
| elder_visits | — | 31.6 | 30.7 | 3796 4223 3663 4441 4044 3940 |
| carers | — | 0.217 | 0.254 | 27 31 29 27 33 31 |
| homeless | 39.1 | 42.8 | 41.0 (+4.9 %) | 5339 4071 4906 5390 4159 3581 |
| evictions | 2.38 | 2.72 | 2.64 (+10.7 %) | 319 238 261 305 254 257 |
| deaths_violence | 1.45 | 1.41 | 1.60 | 133 149 197 183 63 225 |
| thefts / arrests | 2.72 / 24.0 | 2.95 / 24.7 | 2.84 / 24.4 | |
| births | 0.224 | 0.217 | 0.254 | 27 31 29 27 33 31 |
| employed_share | 0.779 | 0.780 | 0.773 | |
| tips | 88.7 | 87.0 | 87.5 | |

- Murders (`analyze_run.py`, 120 d, per seed 42-47): main 85, 89, 197, 148, 19, 94 (mean 105); fix 74, 115, 132, 132, 29, 196 (mean 113). J24 / J25 per seed, main → fix: 42 0.764/0.023 → 0.759/0.025; 43 0.770/0.021 → 0.777/0.019; 44 0.770/0.017 → 0.758/0.023; 45 0.796/0.020 → 0.764/0.025; 46 0.777/0.019 → 0.785/0.018; 47 0.792/0.021 → 0.795/0.016.
- Homeless/evictions cause (the 30-minute look, `scratchpad/p2/evict_cause.py`): a late-run drift, not a mechanism. By 30-day window the fix/main ratio of evictions reads 1.01, 1.11, 1.02, 1.24 and of homeless 0.96, 0.97, 0.97, 1.28, while rent paid, wages, employment, prices and hunger hold at 0.98-1.00 throughout; the first mover is hotel nights (×0.77 from day 0: an evening at home is not an evening at a hotel), then wallets (×0.90 at days 60-90, ×0.85 at 90-120) beside deaths by violence ×1.39 and rough sleeps ×1.13 in the last window; no mechanism of the phase touches rent; printed, not retuned.
- 20k (first build, seed 42 x 30, days 5+; not re-run for the fix): homeless 320 → 309; deaths_violence 4.12 → 3.84; hangouts 3,190 → 3,430; hangout_contacts_mean 1.12 → 2.27; home 1,920, friend 371, home_with_spouse 548, empty_leaves 1,010 a day; batch ticks/s 226 → 217.
- Save size on, 30 d seed 42: 12,777,695 B against main's 12,398,418 (+3.1 %).

## Lives probe (`shadow --lives`, 2k, 42-44 pooled, day 19, 7 d; main → first build → fix)

| arch | n | idle h/d | married | never named | never co-loc | named/wk | co-loc h/wk | hang/wk | empty |
|---|---|---|---|---|---|---|---|---|---|
| all | 1896 | 4.05 → 4.54 → 4.12 | 1489 / 1489 / 1547 | 1016 → 988 → 973 | 473 → 477 → 483 | 1.87 → 3.34 → 3.55 | 22.94 → 23.62 → 23.80 | 14.32 → 16.16 → 15.38 | 0.27 → 0.30 → 0.24 (6831/29029) |
| worker | 90 | 3.43 → 4.38 → 3.59 | 79 | 75 → 71 → 76 | 43 → 35 → 36 | 0.09 → 1.04 → 0.27 | 6.72 → 4.92 → 4.37 | 14.61 → 15.58 → 14.31 | 0.30 → 0.43 → 0.38 |
| office_worker | 90 | 4.78 → 3.77 → 3.80 | 77 | 75 → 56 → 53 | 41 → 39 → 31 | 0.30 → 3.16 → 3.77 | 17.75 → 18.49 → 18.65 | 5.04 → 21.97 → 15.32 | 0.23 → 0.19 → 0.15 |
| night_shift | 90 | 2.61 → 3.38 → 3.11 | 86 | 32 → 31 → 32 | 18 → 12 → 18 | 5.91 → 9.30 → 7.71 | 39.92 → 41.48 → 41.01 | 21.70 → 23.74 → 22.99 | 0.08 → 0.10 → 0.09 |
| cook | 90 | 3.98 → 4.12 → 4.21 | 82 | 40 → 37 → 46 | 14 → 14 → 18 | 5.27 → 5.62 → 6.66 | 41.69 → 43.86 → 39.33 | 19.54 → 18.49 → 19.08 | 0.22 → 0.25 → 0.18 |
| fabber | 90 | 3.81 → 3.68 → 3.49 | 83 | 68 → 54 → 51 | 25 → 30 → 25 | 0.81 → 3.82 → 3.88 | 26.90 → 31.39 → 29.18 | 10.68 → 17.82 → 14.78 | 0.33 → 0.23 → 0.18 |
| guard | 90 | 4.08 → 4.34 → 4.10 | 70 | 43 → 44 → 51 | 24 → 28 → 29 | 0.37 → 3.66 → 1.50 | 2.44 → 11.03 → 5.20 | 14.09 → 15.25 → 13.82 | 0.34 → 0.27 → 0.27 |
| elder | 90 | 4.60 → 5.68 → 4.38 | 71 | 54 → 56 → 64 | 27 → 21 → 32 | 0.07 → 0.56 → 1.11 | 6.02 → 4.97 → 8.16 | 15.78 → 18.41 → 17.52 | 0.38 → 0.47 → 0.41 |
| super | 90 | 3.54 → 5.03 → 3.67 | 50 | 45 → 43 → 48 | 16 → 14 → 17 | 0.00 → 0.32 → 0.36 | 1.93 → 3.89 → 2.90 | 17.98 → 16.01 → 16.99 | 0.27 → 0.50 → 0.29 |
| gang_member | 87 | 3.44 → 4.31 → 3.74 | 76 | 31 → 35 → 33 | 15 → 15 → 25 | 1.17 → 2.09 → 4.22 | 16.54 → 21.22 → 21.78 | 14.91 → 14.51 → 15.05 | 0.37 → 0.38 → 0.20 |

The full fix-tree 2k table is § 13's second table. The first build before its own two fixes (re-plan at once, friends' Homes for every candidate) had read HangOuts 355 → 775 a day, empty 0.27 → 0.73 and 20k ticks/s 226 → 119.

## Throughput (first build; the fix adds one `hunt::habit` read per bodied resident at the pick)

- Alternating A/B, seed 42 x 30 d, one run at a time, 6 rounds: main 6373 / 6182 / 6287 / 6312 / 6587 / 6252 (mean 6332); phase 2 5663 / 5948 / 6238 / 5970 / 5372 / 5242 (mean 5739, −9.4 %). The phase 2 binary with `CITYSIM_HOUSEHOLD_OFF=1` in rounds 4-6: 6338 / 5706 / 5133, a −19 % drift under the same binary and config, so the A/B is noise-bound on this box. 20k 10 d: 251 vs 239 (−4.8 %).
- Profiled (`--features prof`, seed 42, days 10-30, the same binary on vs off, mean of 2): total 5.05 → 5.20 s (+2.9 %). `tick::exec` +0.14 s, `household` midnight 0.049 s over 20 days, `tick::living` +0.04 s, `goal::Visit` 0.016 s. The per-tick `empty_leave` question lost its Vec (`any_inside`).

## Diaries (fix tree: `shadow --pick worker,office_worker,night_shift,cook --count 2 --days 7 --start-day 19`, `scratchpad/p2/fix3/diaries/`; the first build's excerpts in `scratchpad/p2/diaries/`)

Across the eight fix-tree diaries: 29 home HangOuts (24 with company, 5 empty) against 73 street HangOuts (38 with company, 35 empty). The first build's two excerpts stand as the phase's diary acceptance (a cook's four hours at home with the spouse 18:49-22:59 and sleep at 23:01; an office worker's three hours at an acquaintance's Block with the affinity +0.10 → +0.25), both mechanisms unchanged by the fix:

```
d23 18:32 [unwind]  UNWIND pick: rung Free, HangOut at Block#187 (Mid West) at spot (59,111) (score 0.17)
d23 18:32 [plan]    PLAN Unwind on Block#187 (Mid West) [GoTo(Home) > HangOut]
d23 18:49 [hangout] HANGOUT at Block#187 (Mid West) for 61 min (ended): 1 other(s) there: Jocasta Voss#823 (known, Spouse, affinity +1.00) [1 known]
d23 19:52 [hangout] HANGOUT at Block#187 (Mid West) for 61 min (ended): 1 other(s) there: Jocasta Voss#823 (known, Spouse, affinity +1.00) [1 known]
d23 20:55 [hangout] HANGOUT at Block#187 (Mid West) for 61 min (ended): 1 other(s) there: Jocasta Voss#823 (known, Spouse, affinity +1.00) [1 known]
d23 21:58 [hangout] HANGOUT at Block#187 (Mid West) for 61 min (ended): 1 other(s) there: Jocasta Voss#823 (known, Spouse, affinity +1.00) [1 known]
d23 23:01 [goal]    GOAL Sleep (was Unwind): score 0.531; runners-up: Unwind 0.454, Idle 0.050
```

```
d19 18:07 [plan]    PLAN Unwind on Block#22 (Spire) [GoTo(TargetHome) > HangOut]
d19 18:07 [action]  ACTION GoTo(TargetHome) @ Corp HQ#444 (Spire) -> Block#22 (Spire), 35 min (done) [target Block#22 (Spire)]
d19 18:42 [hangout] HANGOUT at Block#22 (Spire) for 61 min (ended): 1 other(s) there: Ghost Kaneda#1024 (known, Acquaintance, affinity +0.15) [1 known]
d19 19:12 [edge]    new relationship with Ghost Kaneda#1024: Acquaintance affinity +0.10
d19 20:15 [hangout] HANGOUT at Block#22 (Spire) for 61 min (ended): 1 other(s) there: Ghost Kaneda#1024 (known, Acquaintance, affinity +0.20) [1 known]
d19 21:16 [edge]    affinity to Ghost Kaneda#1024 +0.10 -> +0.20 (Acquaintance)
d19 21:18 [hangout] HANGOUT at Block#22 (Spire) for 61 min (ended): 1 other(s) there: Ghost Kaneda#1024 (known, Acquaintance, affinity +0.25) [1 known]
```

## Test changes (each flagged)

1. `tests/leisure.rs::test_unwind_skips_paid_rungs_when_broke`: a Free pick may carry a Home as its venue (asserted a Home kind, never a paid venue).
2. `tests/person.rs::probe_night_porter_on_shift_at_two_is_paid_by_eight`: pinned to `household.enabled = false` (on: porter #899 at the Hall on `Found` at midnight, home to bed, off to the Hotel at 01:30; no household step).
3. `tests/person.rs::probe_worker_sleeps_the_night_before_a_shift`: pinned likewise (on: 1 of 20 farmers at a far Braindance Parlour, a five-hour walk home, a nap at 08:23; no household step).
4. `tests/person.rs::probe_gun_never_takes_a_hit_on_kin`: pinned likewise (fix round; on, the day-2 city's first married civilian #590 is killed on day 3 before the hit resolves: `run_to` panics "agent is gone").
5. `tests/succession.rs::test_bug8_leader_by_tenure_and_no_succession_for_a_night`: pinned likewise (fix round; on the household's day-12 city no gang of four has a leader and two free members for the pick).
6. `tests/utility.rs::test_worked_example_through_goal_table`: GOAL_ORDER 31 → 32 (Visit).
7. `citysim-cli/tests/cli.rs`: the header string and `+ 9` columns.

## Parity (run once on the first build, no fix; the fix moves the Full tier the same way)

`test_full_vs_statistical_within_15pct` (234 s) fails on five rows: `hunger-days Full 1.269 vs Statistical 1.779`, `paid venue visits 12.475 vs 16.391`, `paid venue coins 54.724 vs 72.461`, `actor share law0 0.899 vs 0.729`, `actor share law1 0.087 vs 0.219`. law0/law1 failed on main (phase 1's run too); the three others passed in phase 1's run and are the Full tier's home evening against a Statistical table not regenerated (§ 6 / D14: the gate's one regen). `encounters/shift 0.541 vs 0.887` prints as phase 1's FINDING. Written into § 14.

## Deviations from the spec text (§ 14 phase 2 block)

Deed names (`Employed`/`LaidOff`/`Born`); the Home spot added per agent in `spots_near_with` (own Home at any distance **and only with company to be had**, friends' within the cap, friends' only for the scored pick); no `home_from` key; "company only" after an empty leave; the business holders read once a midnight pass; reunite without a queue; the carer's damping after scoring; the business scan at midnight; visits via `GoTo(Victim)` + `Rest`; the CSV home is `Jobs2Cols` (9 columns); the spot tests in `tests/household.rs`. The brief's optional "home share of chats" parity row was not added (`home_hangouts / hangouts` is the share).

## Open items for the orchestrator

- The mirror question: should a Statistical spouse count as home by the clock for the Full tier's evening (and the lives probe's company)? Today it cannot be there, so the device moves only for couples with two moving bodies; the "due" clause is right one time in ten even for bodied residents.
- The parity rows (hunger-days, paid venue visits and coins) stay RED until the gate's regen; an OPEN row is the orchestrator's call.
- Homeless +5 % / evictions +11 %: a late-run (days 90-120) drift with rent, wages and employment flat; printed.
- `scratchpad/p2/` holds the base/off/on/fix/fix2/fix3 runs, the A/B and profile logs, the diaries and the scripts; the worktree's `target/` is for the orchestrator to delete at merge.
