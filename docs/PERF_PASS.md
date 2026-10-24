# The perf pass (spec, 2026-10-24)

The island pass closed with the island holding 20k. Dylan set the game city at 100k (2026-10-23, `OPEN-184`), and
`docs/POPULATION_STUDY.md` measured what stops it: 100k runs 9.5 ticks/s, cost grows ~N^2 because each body's
think scans city-wide building lists, a 21:00 tick stalls for 17 s at 80k, and the world build takes 8 minutes. This
pass makes 100k run, and nothing in it is meant to change what the city does, except phase 1 and the re-import.
Its gate is a quiet-box measurement at 20k, 60k and 100k, not a shadow read. Shadow V9 at 100k on the re-imported
island is the next milestone's job and needs this pass first: on today's binary a 90-day 100k run is ~3.8 h a seed.

## Decisions

| # | decision | by |
|---|---|---|
| R1 | **The index work is byte-identical.** Items 1-5 of the study's roadmap return the same candidate, by the same key and tie-break (door distance, then id), from a superset the grid or cache returns, with no new draw. A phase proves it by save-hash identity against its base, not by an off switch: byte-identical phases carry no switch (doctrine 6 covers behaviour; here the base binary is the off state). | Dylan 2026-10-24 |
| R2 | **Parallel scoring (item 7) is in this pass, last**, after a quiet-box checkpoint on items 1-5. 100k plus a view needs it: items 1-5 are projected to give 100k ~25 ticks/s against 21 for 30× fast-forward, before the view's share. Its proof is save-hash identity across thread counts (1, 2, 8) and 1 thread against the base. | Dylan 2026-10-24 |
| R3 | **The tiers stay at 2k and 20k.** 100k is read at the gate (the bench, the counters, the profile) and by shadow passes, never by a test tier; a 100k build alone is minutes. CLAUDE.md's acceptance line says so. | Dylan 2026-10-24 |
| R4 | **The co-worker crew cap (item 6) is not in this pass.** It changes behaviour (who befriends whom off camera), so it goes to the pressure pass as a realism change with a regenerated stat table (`OPEN-205`). Its cost was overstated: the study's "Friend edges −43 %" predates the El crew cap; `SCALE_20K` re-measured −3 % at 2k and +8 % at 20k. | Dylan 2026-10-24 |
| R5 | **The re-import reports, it does not fix density.** The `k = 2` import at 100k gives 1,428 of 8,000 wanted Lots. The re-import phase measures what `k = 2` and `k = 1` each give at 100k (Lots, Farms, employed, homeless) and commits the better one; whether the island's footprint suits 100k, and how much height to add, is a design question with the graphics side (`OPEN-204`). No knob is retuned to make 100k employ. | Dylan 2026-10-24 |
| R6 | **Not in this pass:** bodies not per capita (item 9, a design call: gangs died without them at S4), the data layout (item 8, the margin if the gate misses: `OPEN-206`), the starting-condition seed (`OPEN-150`; it may replace `preroll_housing` later, but tests need a fast build now, so item 2 goes ahead). | Dylan 2026-10-24 |

## Phases

One fresh Opus coder per phase, ~300 turns, by the plan-implementation SOP. Phases 2-7 are byte-identical
(R1, R2); 1 and 8 change behaviour. Each anchor is the study's (§ Where the time goes, § Optimisation roadmap).

| # | phase | OPEN |
|---|---|---|
| 0 | Housekeeping. Retire the island pass's fifteen switches after one all-off identity against the pass base on seeds 42-44 x 30; keep `[routes] access_cap_ticks` at 0. Bring the change tier under 4m: drop the retired switches' tests, fold new unit tests into shared runs | 201, 202 |
| 1 | The Rest loop: `routine::must_leave_for_work` false when the Work goal cannot run (sick, the shift past reach), one Rest block instead of 1-minute Rests; "no Hq" on the goal line. A behaviour fix, so it has a switch (`CITYSIM_REST_OFF`), retired at the pass close | 195 |
| 2 | Item 1, the venue index: a per-kind `spatial::BuildingGrid` behind `leisure::nearest_venue`, nearest first, stopping at the first open venue that passes. Removes the 21:00 `stat_daily` stall and most of `goal::Unwind` and plan's spot scoring | 184 |
| 3 | Item 2, the world build: `wages::seed_staff` from the `HirePool` `job_search` already builds, not the per-vacancy `pick_candidate` scan; `preroll_housing`'s `reloc::pick` and `free_beds_near` on a Home grid by door. Proof includes a day-0 save identity at 100k | 184 |
| 4 | Items 3 and 4, the price-keyed caches (prices move at midnight): `assets::shop_choice`'s per-kind lowest `seller_level` cached daily, `nearest_seller` on the grid, the think's `shop_choice` reused by `already_satisfied`; `World::local`'s Market and Bar nearest tables rebuilt at midnight, and Court's occupant sum a counter kept by `enter_building` / `leave_building` | 184 |
| 5 | Item 5: `life::claimed_suspects` as one set a tick (memoised like `located_suspects`) or kept on Arrest install and escort; `needs`' per-tick `guard_tiles` sort as a guard grid | 184 |
| — | **Checkpoint** (the orchestrator, not a coder): the bench at 20k, 60k and 100k, two reps, on a quiet box, against the pass base. Records what is left hot for phases 6-7 | — |
| 6 | Item 7a, parallel think: goal scoring reads the world and writes only the agent's brain; score the tick's think slice in parallel (rayon, a new dependency) and apply the results in id order. Every scorer's hidden write (memo caches, the plan queue) moves to the apply step. Thread count from config (`[perf] threads`), 1 by default until the gate | 184 |
| 7 | Item 7b, the Statistical hour in two steps: each agent's rolls on its own keyed stream (`SimRng::agent`), then the writes (edges, wallets, holes) in id order. Same proof as 6 | 184 |
| 8 | The P9 re-import (`OPEN-182`): TheCityGraphics pushed first (`855d0aa`), a venue on its own lot (`B ... lot=<id>`), execs' homes beside their Hq (ISLAND_PASS I9); re-import the 20k tier map, and the 100k map at `k = 2` and `k = 1`, reported side by side (R5). The 20k FINDINGs move for a known reason: write it in the commit, re-baseline, retune nothing | 182, 204 |
| gate | Quiet-box measurement (§ The gate) | — |

**Ordering.** Phase 1 goes before the identities so they run against a base with the Rest fix in. The re-import
goes last so every byte-identical phase proves itself on the same map. The checkpoint sits between the
caches and the parallel work because the remaining profile decides what item 7 is worth.

## The proof, per phase

Light, per the SOP. One `phase_measure.py` call does the 2k part.

- **Byte-identical phases (2-7):**
  - `phase_measure.py --base <sha> --identity` on seeds 42-44 x 30 at 2k;
  - a save-hash identity on the island, 20k seed 42 x 30 (`isle_m_k2`, `--scaled`);
  - the phase's own site at 60k seed 42 x 3 days, `--features prof`, base -> branch in ms a game day, and ticks/s
    by alternating A/B (print only; the box is shared).
  - Phase 3 adds a day-0 save identity at 100k and the build wall at 60k and 100k.
  - Phases 6 and 7 add the save hash at 1, 2 and 8 threads (2k 42-44 x 30, 20k 42 x 30) and 1 thread against the
    base.
- **Phase 1:** the off identity on 42-44 x 30 with `CITYSIM_REST_OFF`, the counters against the base, and the device
  rows: 1-minute Rest plans a body-day and idle hours for the diaries `OPEN-195` names (scav, manager, super).
- **Phase 8:** the 20k tier map's counters base -> branch (seed 42-44 x 30, island); the 100k table of R5.
- **Every phase:** `test.py change` once at the end; the coder writes its report to the scratchpad path in the brief.

## The gate

On a quiet box (no Blender renders, no parallel builds), against the pass base `6d6f7bc`. Asserted, as tps floors
and identities (doctrine 1):

- **100k steady ≥ 30 ticks/s** on the committed 100k map (30× fast-forward is 21 ticks/s; the rest is the view's
  share), at the thread count the gate sets for `[perf] threads`.
- **No tick over 1 s at 100k**, the 21:00 and midnight passes included (at X1, 0.7 ticks/s, a longer tick freezes the
  view).
- **The 100k build under 2 minutes.**
- **Thread-count identity** (phases 6-7) on 42-44 x 30 at 2k and 42 x 30 at 20k.

Printed, never asserted:

- the bench at 20k, 60k and 100k, two reps (the study's table re-run);
- the profile at 100k (the study's site table, base -> close);
- the Full budget at 100k measured, not derived: `max_full` raised on the 100k map, as `OPEN-149`'s probe did at 20k;
- 100k seed 42 x 30 counters and the 20k ones on the re-imported map.

**If 100k misses 30 ticks/s:** the data layout (`OPEN-206`) is the next lever, as its own pass; the gate records the
miss and the gap, and the target is not lowered to pass.

## Statistical mirrors (doctrine 7)

Phases 2-7 change no mechanism, so there is nothing to mirror: an index returns what the scan returned at both tiers.
Phase 1 is Full-tier only: a Statistical agent has no plan queue and no Rest action, so it has no mirror. Phase 8
changes the map, not a mechanism.

## Implemented: deviations

(Written at the close.)
