# Shadow V7, phase 1: the windows and the measurement set (runs report, 2026-10-22)

Branch `worktree-agent-a996c55984dfa76d3` on `92c4a37`; base `4fb10b9` (pre-pass main). Diaries stay in the worktree for
the critics; every path below is absolute. V6's diaries went with their worktree, so every V6 number here is from
`shadow_v6_runs.md` and `SHADOW_V6.md`, not a re-read of its `camera.tsv`.

**Tool changes** (`citysim-cli/src/shadow.rs` only, read-only, no RNG; the module header lists them):
- `Break`, `Unload` and `Court` read as work (they were `other`), and so does any leg of a shift's shape plan (a haul trip, the walk out to the break).
- The escort van, a cab driver's fare leg and a rider aboard a body cab read as vehicle. Their jump allowance is the cab's timed speed (`move_ticks_full` x the Car's `timed_mult`); V6 read them as a street walk at 1 tile a tick.
- `camera.tsv` gains three columns a day: `work_longest_min` (the longest run of work-class minutes on one plan step), `work_tiles` (distinct tiles at work minutes) and `goto_over_120` (GoTo steps over 120 min). The diary's camera notes carry them too.
- `--lives --population N` reads `--map`, and its city label becomes `20k@isle_m_k2`.
- `--pick arch:N` picks N of one archetype.
- Unchanged: a corpse's minutes still count as an unchanged stretch (2 of the long-still days below are corpses). The `full_cab` commuter (pre-pass) still reads as street. The order lines and `[off]` needed no fix (events naming the pick print as they are; `[off]` came with phase 4).
- **Guard**: `phase_measure.py --base 92c4a37 --off CITYSIM_SHADOW_V7_OFF` (a no-op switch), seed 42 x 30: events identical (26,201 lines), 1,073 report columns identical, save identical. Tool unit tests 4/4 pass; clippy and fmt clean.

## Commands and wall times

`B=target/shadow_v7/bin/citysim-cli.exe` (this tree's release build). `A` = V5's 48 archetypes with `gang_leader:4,sweeper:4`. `ISLE=--population 20000 --map assets/maps/isle_m_k2.txt` (shadow applies the scaled config and 50 Ambient flats). Scripts: `target/shadow_v7/run_windows.sh`, `rollup.py`, `trades.py`, `isle_counters.py`, `ab.py`, `parity_bisect.py`. Eleven window jobs ran together beside the 2k measurement set and the island counters, so these are loaded-box wall times.

| window | command | wall s (`--list`) | diaries / pick-days | archetypes with no candidate |
|---|---|---|---|---|
| `isle_s42_d0` | `shadow $ISLE --seed 42 --start-day 0 --days 7 --count 2 --pick $A` | 113 (44) | 58 / 406 | **19** (as V6): gang_member, gang_leader, runner, purist, dealer, sweeper, gun, buyer, joytoy, street_worker, pimp, vendor, day_labourer, office_worker, arms_dealer, companion, black_doc, cultist, scav |
| `isle_s43_d30` | `... --seed 43 --start-day 30 ...` | 628 (452) | 92 / 644 | day_labourer, arms_dealer, black_doc; 1 dealer, 1 pimp |
| `isle_s44_d90` | `... --seed 44 --start-day 90 ...` | 1471 (1299) | 95 / 659 | day_labourer, arms_dealer; 1 pimp |
| `gen20k_s43_d30` | `shadow --population 20000 --seed 43 --start-day 30 --days 7 --count 2 --pick $A` | 611 (407) | 96 / 669 | arms_dealer, black_doc |
| `k2_s42_d26` | `shadow --seed 42 --start-day 26 --days 7 --count 1 --pick $A` | 20 (12) | 48 / 334 | dealer, buyer, pimp, arms_dealer, black_doc (the fixer was taken by another pick) |
| `isle_s43_d30_nopin` | `shadow $ISLE --seed 43 --start-day 30 --days 3 --count 1 --no-pin --pick <V6's 12>` | 500 | 12 / 36 | - |

Candidates on the island against gen at d30 (seed 43): homeless 201 / 273, gang members 194 / 541, leaders 26 / 40, sick 146 / 151, cabbies 48 / 66. **At island d90 (seed 44), 2,188 are sick** (gen d30: 151) and 536 are homeless.

Diary directories (`.md` + `.jsonl` per pick, `camera.tsv`, the window's stdout in `<window>.log` beside it):
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\isle_s42_d0`
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\isle_s43_d30`
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\isle_s44_d90`
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\gen20k_s43_d30`
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\k2_s42_d26`
- `D:\TheCity\.claude\worktrees\agent-a996c55984dfa76d3\target\shadow_v7\isle_s43_d30_nopin`
- Roll-ups: `...\target\shadow_v7\camera_rollup.md` (per window, and pooled by archetype) and `trades.md` (beats per archetype per window: fares, parcels, Court, OrderGiven, Unload, Break, `[off]`, SETTLED APART, jumps).

## The camera, per window (means per pick-day; hours unless marked; V6's like-for-like row in brackets)

| window | home | work | street | veh | held | plans | aborts | jumps (body) | awake still m | settles (apart/stat/asleep) | longest work m med/max | work tiles | GoTo>120 /pd |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| isle_s42_d0 [k20_s42_d0] | 8.3 [9.1] | 5.2 [5.7] | 7.4 [6.2] | 1.9 [0.3] | 0 [0.6] | 11.0 [11.0] | 1.4 [1.8] | 0 (0) [5 (0)] | 176 [386] | 1.7 (0.0/1.1/0.1) [3.4 (0.6/2.2/0.3)] | 89/538 | 16.4 | 1.09 |
| isle_s43_d30 [k20_s43_d30] | 6.9 [8.4] | 4.0 [5.1] | 8.8 [7.5] | 2.0 [0.4] | 0.7 [1.1] | 9.9 [9.9] | 1.5 [1.7] | 15 (10) [15 (3)] | 224 [413] | 2.2 (0.0/1.4/0.0) [3.8 (0.2/2.7/0.1)] | 89/538 | 18.5 | 1.20 |
| isle_s44_d90 [k20_s44_d90] | 6.9 [7.9] | 3.6 [5.0] | 8.2 [7.3] | 1.9 [0.6] | 1.8 [1.0] | 8.9 [10.0] | 1.7 [1.7] | 15 (0) [14 (5)] | 295 [371] | 2.6 (0.0/2.3/0.0) [3.4 (0.2/2.9/0.2)] | 88/539 | 23.6 | 1.13 |
| gen20k_s43_d30 [k20_s43_d30] | 7.4 [8.4] | 4.9 [5.1] | 8.1 [7.5] | 0.8 [0.4] | 1.0 [1.1] | 11.4 [9.9] | 1.9 [1.7] | 8 (4) [15 (3)] | 233 [413] | 2.9 (0.1/2.3/0.3) [3.8 (0.2/2.7/0.1)] | 90/538 | 13.7 | 0.90 |
| k2_s42_d26 [k2_s42_d26] | 8.1 [7.8] | 4.2 [4.5] | 7.2 [7.2] | 0.5 [0.3] | 2.6 [2.3] | 11.5 [10.0] | 1.6 [1.7] | 2 (0) [9 (0)] | 305 [453] | 2.7 (0.0/2.1/0.0) [3.9 (0.2/3.2/0.1)] | 90/538 | 16.2 | 0.60 |
| nopin [nopin] | stat 19.4 h, coarse 1.0 [16.0, 6.0] | | | | | 2.4 [3.8] | 0.6 [0.8] | 0 [1 (1)] | 1215 [1049] | 0.5 | - | - | 0.25 |

- **Pooled over the four pinned windows** (2,043 pick-days against V6's 1,994): street 8.0 h (7.1), vehicle 1.7 h (0.4), held 1.2 h (1.1), plans 10.0 (10.2), aborts 1.5 (1.7), awake still 251 min (400). Aborts 3,154: Interrupted 2,765 (88 %), Unplannable 182, PreconditionLost 166, BuildingFull 16, PartnerLeft 17, NoSuchPlace 4, StockGone 4.
- **nopin**: 9 of 12 picks were Statistical all 3 days, and a street worker for 49 h; the gang member and the guard were never Statistical. The awake still is 1,215 min (the snapshot body, OPEN-149, unchanged by design; V6: 8 of 12).

## Rows 1-7 against V6 (the camera, pooled pinned windows unless named)

1. **The shift as one statue action.** V6: ClerkWork blocks of 530-540 min on one tile in 55 % of labour pick-days. V7 labour work-days (874): longest single work action median **89 min**; over 180 min on 228 (26 %); one work tile on 189 (22 %). The residue, by archetype (median longest, days over 180 / work-days):
   - fabber 537 (40/41, one tile on 36): `FabWork` is still one block.
   - super 334 (36/37).
   - sweeper 271 (35/57, one tile on 45; island `sweep_stands` 186 a seed beside 325 rounds).
   - cook 224 (31/43); madam 192 (24/40); bouncer 186 (24/42, the door counter).
   - cabbie 118 (one tile on 39 of 39: standing by).
2. **The idle freeze.** Awake still of 12 h or more: 45 of 2,043 pick-days, and 35 of them are in jail. The other 10:
   - 5 are one buyer's Meetings of 13-14 h at a Corp HQ (`isle_s44_d90/buyer_9020`).
   - 2 are a fixer's GoTo standing 771-773 min on one street tile, then landing in the Spire (`isle_s42_d0/fixer_23879` d5-d6: a 896-min commute).
   - 2 are corpses (the tool's own limit). 1 is a captive scav held 3.5 days at a Hideout.
   - V6 had 29 such days in 14 of 33 street diaries alone. No silent Earn freeze is left in the diaries.
3. **Settlement by presence.** Settlements naming a person: 4,703 (V6 7,153). Statistical 75 % (76 %), body apart **1.0 %** (8 %), pick asleep **1.1 %** (10 % at d0, 1-5 % later; island d0 now 3.0 %).
   - Gen d30 reads 11 % asleep: one dancer's `Finance` flow lines (211), a ledger flow, not an encounter.
   - The remaining SETTLED APART lines are wage, hotel and `Finance` flows, and a few door, source and patient memories.
4. **Long legs.** GoTo steps over 120 min: 2,159 in 2,043 pick-days (island 1.09-1.20 a pick-day, gen 0.90, 2k 0.60; V6 had no column). Jumps 32, body jumps 10 (V6 44 / 9). 9 of the 10 are one exec (`isle_s43_d30/exec_19911`) on `exec_ride`, a timed ride: the body stands at its Spire Home for the ~282-min ride, then lands 300-316 tiles away. The escort jumps are gone (V6 4): island vans 2,414 and walks 748 a seed.
5. **Housing churn.** No double `Housed` at one tick in any diary. Exec, CEO and guard picks never home at 20k: 1 of 24 (`isle_s44_d90/guard_18189`; V6 5 of 14). `NoSuchPlace` 12 lines in 5 diaries (guards' `GoTo(Home)` and `GoTo(PatrolWaypoint)`, an elder, a homeless). `GoTo(Hotel)` resolves to a Soup Kitchen 27 times: homeless picks, plus a builder, a guard and a worker_friday.
6. **Dead trades** (diaries, `trades.md`):
   - cab: 0 fares on the island, 2 each at 2k and gen (`[off]`).
   - courier: 0 parcels handed on the island. All 265 of the island couriers' `Unload`s target their own Freight Yard, so the haul runs inside the post. Gen hands over 10.
   - bouncer: 0 refusals. joytoy: 0 clients.
   - leader: Court 12-28 a window, `OrderGiven` 1-4, `Protected` 0-2.
   - scav: harvests 3-11.
7. **The implant loop.** 1 `BuyAsset` PreconditionLost in 401 diaries, failed at 1 min (validated before the wait); 49 buys completed. Fixed.

## The devices

- `shadow --assert`: **132 PASS, 0 FAIL** (74 s). V6 had 133; phase 5 demoted the d19 sweeper WalkHDay row.
- `shadow --lives --population 20000`, the `all` row. V6 (gen, `a25fdc5`): n 220, idle 3.95 h/d, named/wk 1.01, empty 0.42, Served-able 0.32, met 0.42, enc/shift 2.69, travel 37.9 h/wk.
  - **gen**: n 220, idle 3.46, named/wk 1.24, empty 0.41, Served-able 0.32, met 0.29, enc/shift 2.57, travel 40.2 (424 s).
  - **island**: n 205, idle 2.93, named/wk 1.38, empty 0.21, Served-able 0.29, met 0.22, enc/shift 2.95, travel **50.6** (397 s).
- 2k `--lives` (42-44, base -> cand), `all`: idle 4.47 -> 4.01, met 0.44 -> 0.28, enc/shift 3.72 -> 1.85. Leaders' met 0.72 -> 0.18; sweepers' met 0.24 -> 0; forgers' met 0.60 -> 0.07 (OPEN-157).

## The measurement set against `4fb10b9`

- **Composed off identity** (all eight pass switches at once, seeds 42-44 x 30): **IDENTICAL** on all three seeds. Events 26,480 / 24,732 / 23,890 lines; 1,027 shared columns (46 new); saves identical with `save_version` 45 -> 51 masked.
- **2k, 42-47 x 120** (base -> cand, means): Murders 70 -> 79, J24 0.776 -> 0.776, J25 0.014 -> 0.014, arrests 2,444 -> 2,546, wallets +2 %, ticks/s 2,495 -> 2,472 (loaded).
  - Parcels 3,614 -> 3,210; served_tenant 100,077 -> 14,720; contract encounters 350 -> 74.
  - Protection paid 48 -> 29; gang income 28,943 -> 28,154; raids 18 -> 18.
  - Orders: 21 at court, 20 walked, 9.5 by word; courts 170; lead rounds 46.
  - Fares by a body 1.2 a seed; cab rides by a body 1 -> 16; escort vans 1,794; sweep rounds 28, stands 0.5.
- **Island 20k, 42-44 x 30** (base -> cand, means): Murders 14 -> 16, employed share d29 0.578 -> 0.585, homeless d29 156 -> 193, starving d29 31 -> 24, arrests 2,342 -> 3,146 (+34 %), wallets -17 %, gang members 185 -> 218, Hires -1 %.
- **Alternating A/B ticks/s** (island 20k, seed 42 x 5 d, quiet box, ABABAB): base 260 (259/260/261), cand 255 (254/254/256), -2 %. Print only.

## The pass's FINDINGs, read on the island at 20k (seeds 42-44 x 30, base -> cand)

- **Murders by seed**: 23 -> 15, 6 -> 16, 12 -> 17. At 2k: 101 -> 63, 116 -> 98, 79 -> 148, 55 -> 70, 51 -> 32, 17 -> 61. Within each set's own spread; printed, not tuned.
- **Wages and employment**: flow_wages +1 %, wage_gross_mean 6.96 -> 6.90, employed 11,576 -> 11,705. At 2k: J24 0.776 -> 0.776.
- **Parcels**: 7,761 -> 7,072 (-9 %), `parcels_missed` 0. The courier diaries hand none over on screen (row 6).
- **served_tenant** 151,422 -> 34,406 (-77 %; 2k -85 %) at `stat_home_p` 0.5; `presence_missed` 65,340 a seed (2k 42,288).
- **Contract encounters** (OPEN-157): 103 -> 49 (-53 %; 2k -79 %).
- **Eat→Earn aborts** (`PlanAborted` "Eat -> Earn"): 829 -> 1,275 a seed (+54 %); at 2k 279 -> 341 (+22 %).
- **Protection and gang income** (OPEN-160, OPEN-162): protection made 1.3 -> 2.0 a seed, paid 1.3 -> 1.7; lead rounds 2, stops 2, owner out 0.3 a seed. **OPEN-162 stands at 20k**: almost no protection, so the round has nothing to collect. Gang income 12,203 -> 11,240 (-8 %).
- **Orders by word** (OPEN-161): 8.7 at court, 12 walked, **14 by word (40 %)**; 2k 19 %. Courts 185, takes handed 2 a seed.
- **Body cab drivers**: `fares_driven` **0** on all three seeds. `fares_no_body` 2,824 and `fares_unmanned` 355 a seed; `cab_rides_driven` 0 -> 0.3.
- **The sweeper, ride against walk**: pooled vehicle 3.6 h a pick-day (V6 1.1), street 10.6 (12.6). Island sweep rounds 325 against 186 stands a seed (36 % stands); 2k 28 rounds, 0.5 stands.

## Milestone footer

`python tools/test.py milestone` (alone on the box): **RED (parity); BUDGET OK 14m29s of 20m00s**. core_20k ok (265 s), god_20k ok (379 s).
- **Parity** (`lod::test_full_vs_statistical_within_15pct`, 225 s) fails on three rows: hunger-days Full 0.578 vs Statistical 1.163; ride share x100 24.2 vs 34.8; paid venue visits 12.3 vs 14.2.
- With all eight switches off it passes, as the composed identity predicts. One switch off at a time (`parity_bisect.txt`):
  - hunger-days clears only with `CITYSIM_IDLE_FIX_OFF`: **phase 0** (OPEN-138).
  - ride share clears only with `CITYSIM_STATIONS_OFF`: **phase 2**.
  - paid venue visits clears with any one of idle-fix, housing, haul/counter, presence or legs off: a sum of phases, no single cause.
- Not fixed (no sim change in V7).
