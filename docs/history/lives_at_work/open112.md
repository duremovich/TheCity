# OPEN-112 fix round: hand-back

Base 30d485d. Branch head **7aec7ed** (fix 1b7fb5b, OPEN.md close 7aec7ed). SAVE_VERSION stays **42**: nothing saved changed.

## Repro and bisect
- With leverage.rs `gang_world` as a clone, `test_save_mid_coercion_round_trips` failed. The first diverging tick was 1422 after the save (world tick 33132, day 23), and the only field that differed was `position[1570]`. Swapping in the loaded world's `flow_fields` made it pass. At the save, 112 of 216 cached fields differed from `FlowField::build`, and every differing tile was inside a building rect.
- A daily sweep (live world; save and load at each day start; run the day; compare saves) found three more on base or partly-fixed trees. Shipped city day 7: `guards_on_take` was 2 vs 1. Day 103: `heard` stores differed at tick 1, traced to the `enemies` index (18 pairs with a dead agent, only after a load). Index diff: `by_party` list order.

## Causes and fixes (all `#[serde(skip)]` or uncached, so no format change)
1. **Flow fields** (M11 `FlowCache::invalidate_sealed`). When a building goes up on a Lot, a kept field left the rect's bytes as the open Lot's. A body inside the rect with `building: None` (a stored path walked through the new walls) stepped by those stale bytes live, and by a rebuild after a load. Fix: `FlowField::reseal` sets the walls to zero and re-runs the interior from the door. It uses the build's Dijkstra (same pop order, strict relax), seeded at the door's cost. That cost is summed back along the descent, which repeats the build's float additions. Kept fields now equal a rebuild, and the no-rebuild optimisation stays.
2. **`on_take`**. It was filtered by `is_city_guard` and `is_corruption` when it was reindexed, and those can change between reindexes. Fix: the index is now taker -> Taken Guard records with no broker. `take_buyers` applies the filters at the read, and so do `on_take_of`, the daily `guards_on_take` and the app (inspector, city panel).
3. **`enemies`**. `social::unlink` drops the dead from the live index, but the load's rebuild kept their Enemy edges. Fix: the rebuild keeps only the living, as it already does for spouses.
4. **`by_party`**: lists are kept in ascending order, as the rebuild builds them.
- **Not touched:** `service_room` (service.rs, OPEN-111's file) also differs live vs loaded. The live store held 143 chars of entries and the loaded one none; the entries were for the previous hour. The mid-day sweep below stayed clean, but a save taken inside an hour that already has bookings may still differ. Worth a look by the service.rs owner.

## Tests (save.rs)
- `test_live_save_resumes_after_lots_are_built`: leverage city, day 23; also asserts every cached field equals a rebuild. Base diverges on day 23.
- `test_live_save_resumes_with_guards_on_the_take`: shipped city, day 7. Base diverges here.
- `test_enemies_index_after_a_death_matches_a_load`: unit test (300-agent city, an Enemy edge, a kill). Expected to fail on base, since the rebuild keeps the dead; not run on base.
- `diag_live_save_sweep` (`tier: diag`, prints a FINDING). Results: 42/43/44 x days 1..120 clean; seed 42 at +700 ticks x 1..60 clean; leverage city 1..60 clean.
- leverage.rs `gang_world` (both variants) is now a clone of the live city. All 21 pass. Binary time 6.67 s before, 6.69 s after: no speed-up.
- `tests/save.rs`: 13 passed, 1 ignored (diag), 5.7 s.

## Off identity
`phase_measure.py --base 30d485d --off CITYSIM_LIVES_OFF --skip counters,lives` reads **DIFFER**, but the tool's base cache is the cause, not this fix. A control (30d485d sources built in this worktree, no changes) gives exactly the same DIFFER: 25835 vs 21744 event lines, 11574 cells, and a different owner name at tick 0. So the cached base `D:\TheCity\target\measure\base-30d485d325e3\off_42x30` doesn't match a fresh build of the same sha; worth checking the tool. Compared directly, fix vs control (both CITYSIM_LIVES_OFF, 42 x 30): events identical (only the save path differs), report CSV identical with ticks_per_sec removed, end save identical with ticks_per_sec masked. **IDENTICAL.**
With Lives on, the fix can move a fresh run. The stale interior bytes and the stale `on_take` entries were live behaviour; now only the canonical state steers the sim.

## Footers
- `test.py change`: `[change] GREEN; BUDGET BREACHED 5m18s of 4m00s`. Clippy ran cold (51.8 s, vs 0.7 s warm in 8b) and core took 54.6 s (40.4 in 8b) on a shared box. My added test cost is about 2 s on the save binary.
- `test.py lint`: 0 problems, 6 warnings (file lengths); ignores 55 (+1, the diag sweep).
- Worktree target/ kept.
