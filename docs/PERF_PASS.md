# The perf pass (spec, 2026-10-24)

The island pass closed with the island holding 20k. Dylan set the game city at 100k (2026-10-23, `OPEN-184`), and
`docs/POPULATION_STUDY.md` measured what stops it: 100k runs 9.5 ticks/s, cost grows ~N^2 because each body's
think scans city-wide building lists, a 21:00 tick stalls for 17 s at 80k, and the world build takes 8 minutes. This
pass makes 100k run, and nothing in it is meant to change what the city does, except phase 1 and the re-import.
Its gate is a quiet-box measurement at 20k and 100k on days 25-30 (R7), not a shadow read. Shadow V9 at 100k on the re-imported
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
| R7 | **After the checkpoint: the slope first, then midnight, then parallel scoring** (the checkpoint found a 30-day slope a 4-day bench hid). Phases 6-7 are inserted; the old 6-8 become 8-10. **No new city versions or tiers for it:** the slope is diagnosed on the 20k tier map (`isle_m_k2`, seed 42 x 30, ~4 min; it shows the slope at 387 -> 210 ticks/s) and read once at 100k per phase, branch only, against the checkpoint's recorded base run. The 60k map retires from the per-phase proof. **No test creep:** a byte-identical fix is proven by the save-hash identity, not a new test; a new index extends an existing scans-vs-index test rather than adding one. State that grows without bound by design (no decay, no cap) is a behaviour question for Dylan, not a fix in this pass. | Dylan 2026-10-25 |

## Phases

One fresh Opus coder per phase, ~300 turns, by the plan-implementation SOP. Phases 2-9 are byte-identical
(R1, R2, R7); 1 and 10 change behaviour. Each anchor is the study's (§ Where the time goes, § Optimisation roadmap).

| # | phase | OPEN |
|---|---|---|
| 0 | Housekeeping. Retire the island pass's fifteen switches after one all-off identity against the pass base on seeds 42-44 x 30; keep `[routes] access_cap_ticks` at 0. Bring the change tier under 4m: drop the retired switches' tests, fold new unit tests into shared runs | 201, 202 |
| 1 | The Rest loop: `routine::must_leave_for_work` false when the Work goal cannot run (sick, the shift past reach), one Rest block instead of 1-minute Rests; "no Hq" on the goal line. A behaviour fix, so it has a switch (`CITYSIM_REST_OFF`), retired at the pass close | 195 |
| 2 | Item 1, the venue index: a per-kind `spatial::BuildingGrid` behind `leisure::nearest_venue`, nearest first, stopping at the first open venue that passes. Removes the 21:00 `stat_daily` stall and most of `goal::Unwind` and plan's spot scoring | 184 |
| 3 | Item 2, the world build: `wages::seed_staff` from the `HirePool` `job_search` already builds, not the per-vacancy `pick_candidate` scan; `preroll_housing`'s `reloc::pick` and `free_beds_near` on a Home grid by door. Proof includes a day-0 save identity at 100k | 184 |
| 4 | Items 3 and 4, the price-keyed caches (prices move at midnight): `assets::shop_choice`'s per-kind lowest `seller_level` cached daily, `nearest_seller` on the grid, the think's `shop_choice` reused by `already_satisfied`; `World::local`'s Market and Bar nearest tables rebuilt at midnight, and Court's occupant sum a counter kept by `enter_building` / `leave_building` | 184 |
| 5 | Item 5: `life::claimed_suspects` as one set a tick (memoised like `located_suspects`) or kept on Arrest install and escort; `needs`' per-tick `guard_tiles` sort as a guard grid | 184 |
| — | **Checkpoint** (the orchestrator, not a coder): the bench at 20k, 60k and 100k, two reps, on a quiet box, against the pass base. Records what is left hot for phases 8-9. Found the 30-day slope (§ Implemented) | — |
| 6 | **The slope.** What grows with the days and what scans it: the profile at 20k seed 42, days 2-3 against days 27-29, per site; the size of every growing store (social edges, memories, holes, contracts, grudges, word, ledgers) per day. A store read by a scan gets an index or a prune that is byte-identical (an entry no rule reads again); growth by design comes back to Dylan (R7). The store sizes that explain the slope stay as report columns (`OPEN-203`'s new-edges counter among them, if the edges are it) | 184 |
| 7 | **Midnight**, what phase 6 leaves of it: `job_search` (`pick_candidate`, `pick_graded`), `living`'s and `corp_brain`'s daily passes, byte-identical. Gate: no tick over 1 s at 100k on days 25-30 | 184 |
| 8 | Item 7a, parallel think: goal scoring reads the world and writes only the agent's brain; score the tick's think slice in parallel (rayon, a new dependency) and apply the results in id order. Every scorer's hidden write (memo caches, the plan queue) moves to the apply step. Thread count from config (`[perf] threads`), 1 by default until the gate | 184 |
| 9 | Item 7b, the Statistical hour in two steps: each agent's rolls on its own keyed stream (`SimRng::agent`), then the writes (edges, wallets, holes) in id order. Same proof as 8 | 184 |
| 10 | The P9 re-import (`OPEN-182`): TheCityGraphics pushed first (`855d0aa`), a venue on its own lot (`B ... lot=<id>`), execs' homes beside their Hq (ISLAND_PASS I9); re-import the 20k tier map, and the 100k map at `k = 2` and `k = 1`, reported side by side (R5). The 20k FINDINGs move for a known reason: write it in the commit, re-baseline, retune nothing | 182, 204 |
| gate | Quiet-box measurement (§ The gate) | — |

**Ordering.** Phase 1 goes before the identities so they run against a base with the Rest fix in. The re-import
goes last so every byte-identical phase proves itself on the same map. The checkpoint sits between the
caches and the parallel work because the remaining profile decides what item 7 is worth.

## The proof, per phase

Light, per the SOP. One `phase_measure.py` call does the 2k part.

- **Byte-identical phases (2-9):**
  - `phase_measure.py --base <sha> --identity` on seeds 42-44 x 30 at 2k;
  - a save-hash identity on the island, 20k seed 42 x 30 (`isle_m_k2`, `--scaled`);
  - the phase's own site at 60k seed 42 x 3 days, `--features prof`, base -> branch in ms a game day, and ticks/s
    by alternating A/B (print only; the box is shared).
  - Phase 3 adds a day-0 save identity at 100k and the build wall at 60k and 100k.
  - From phase 6 (R7): the site's profile at 20k seed 42 x 30 replaces the 60k one, and one 100k seed 42 x 30
    run on the branch (ticks/s and the longest tick per day) is read against the checkpoint's base run.
  - Phases 8 and 9 add the save hash at 1, 2 and 8 threads (2k 42-44 x 30, 20k 42 x 30) and 1 thread against the
    base.
- **Phase 1:** the off identity on 42-44 x 30 with `CITYSIM_REST_OFF`, the counters against the base, and the device
  rows: 1-minute Rest plans a body-day and idle hours for the diaries `OPEN-195` names (scav, manager, super).
- **Phase 10:** the 20k tier map's counters base -> branch (seed 42-44 x 30, island); the 100k table of R5.
- **Every phase:** `test.py change` once at the end; the coder writes its report to the scratchpad path in the brief.

## The gate

On a quiet box (no Blender renders, no parallel builds), against the pass base `6d6f7bc`. Asserted, as tps floors
and identities (doctrine 1):

- **100k steady ≥ 30 ticks/s on days 25-30** (R7) on the committed 100k map (30× fast-forward is 21 ticks/s; the rest is the view's
  share), at the thread count the gate sets for `[perf] threads`.
- **No tick over 1 s at 100k on days 25-30**, the 21:00 and midnight passes included (at X1, 0.7 ticks/s, a longer tick freezes the
  view).
- **The 100k build under 2 minutes.**
- **Thread-count identity** (phases 8-9) on 42-44 x 30 at 2k and 42 x 30 at 20k.

Printed, never asserted:

- the bench at 20k and 100k, two reps, 30 days (the study's table re-run);
- the profile at 100k (the study's site table, base -> close);
- the Full budget at 100k measured, not derived: `max_full` raised on the 100k map, as `OPEN-149`'s probe did at 20k;
- 100k seed 42 x 30 counters and the 20k ones on the re-imported map.

**If 100k misses 30 ticks/s:** the data layout (`OPEN-206`) is the next lever, as its own pass; the gate records the
miss and the gap, and the target is not lowered to pass.

## Statistical mirrors (doctrine 7)

Phases 2-9 change no mechanism, so there is nothing to mirror: an index returns what the scan returned at both tiers.
Phase 1 is Full-tier only: a Statistical agent has no plan queue and no Rest action, so it has no mirror. Phase 10
changes the map, not a mechanism.

## Implemented: deviations

- **Phase 0** (`c86aa55`, `OPEN-201` closed, `OPEN-202` left open for F3's A/B at the checkpoint). The all-off
  identity against `fa74b64` (42-44 x 30) DIFFERed on all three seeds through one cause, `0d61e91` (the stat table
  regenerated after the El trains, outside every switch by design): with `fa74b64`'s table in the tree the three
  seeds are identical (events, report, save). The fifteen env reads retired; every section keeps its config path
  (absent: the base rule) and SAVE_VERSION stays 67; identical to `3728aca` on 42-44 x 30. **Deviations:** no test
  read an env switch, so none was switch-only; the phases' off halves (a section dropped in-test) stay, since the
  config paths stay and they cost under a second (the island pass phase 0 removed its off halves). Six scene pins
  waiting on the retirement were re-read: five pass on the shipped city and went; the super probe keeps `[distance]`
  absent (293 min a shift, `OPEN-169`). The change tier came under budget not by cutting tests but by running the
  binaries six at a time in `tools/test.py` (tests step 223 s -> 87 s; the tier 1m33s warm, quiet box).
- **Phase 1** (`71fee8c`, `OPEN-195` closed, `[rest]`, `CITYSIM_REST_OFF`). The root was wider than F3's: three
  families of one mechanism, a Rest cut by a work signal the next think does not answer. (1) The departure gate
  ignored the Work goal's refusals: `routine::work_refused` (a sentence, a sick day, a week unpaid) is now shared by
  Work's `in shift` gate and the gate. (2) The gate cut Rests it had already seen: a rich manager's Work scores under
  Idle, and the sweeper's commute Rest at the beat is latched; `routine::leave_gate_at` reads the gate at the Rest's
  start, and a Rest begun with it open runs (the 30-tick think still lets Work win). (3) `can_work_now` ignored the
  rest day (the super at his own block); it now cuts only on a due shift, and not an idle Rest begun on it at the
  post. **Deviations:** "share the predicate" is a new `work_refused` both read, not the Work goal's whole score
  (Work losing on score cannot be a predicate, hence rule 2); the exec's 0-Work diaries are a Home-only corp with no
  office (`OPEN-207`, the goal line says "Work: no Hq"), and the manager's idling is a scoring question (`OPEN-208`);
  both left. The device row is a new `shadow --lives` column, `short_rest/d` (an idle Rest cut within 5 min), read
  on the base by running the branch with the switch off (identical to the base by the off identity): 2k 42-44
  pooled 0.16 -> 0.02 (super 0.89 -> 0.02, gun 0.49 -> 0.02), idle 4.46 -> 4.26 h/d. `person.rs`' loader-rehire
  probe now seats its two candidates in the Home nearest the Yard (a third, nearer jobless adult took the post once
  the city moved). SAVE_VERSION stays 67.
- **Phase 2** (`8b5956d`, `OPEN-184` annotated). `leisure::nearest_venue` and `evening_venues` read a per-kind
  `spatial::BuildingGrid` (`World::kind_grid`) through `BuildingGrid::first_by_distance`: rings outward from the
  origin's box, the candidates tested in the old sort's `(door distance, id)` order up to the first pass. Kept
  current by membership only (every gate, "open" included, is read at query time; doors never move):
  `buildings_by_kind` is private and written only through `World::buildings_by_kind_mut()`, which bumps
  `kinds_gen`; the grids are keyed by `(building_gen, kinds_gen)`, not saved. Identical to `acacea1` on 2k 42-44 x
  30 and 20k `isle_m_k2` 42 x 30. At 60k seed 42 (ms a game day): `goal::Unwind` 7,927 -> 1,847, `stat_daily` (the
  21:00 tick's call) 7,944 -> 527, `tick::plan` 1,847 -> 1,090, the tick total 46,660 -> 33,820 (-28 %); 20k ABAB
  272 -> 314 ticks/s (print only). **Deviations:** two callers, not one: `stat_daily`'s learned evening goes through
  `evening_venues`, its own collect-and-sort keyed `(other district, distance, id)`, now two searches (the Home's
  district, then outside it). Both cut the search at the walk cap's tiles (`exec::walk_reach_tiles`, the
  litter-free floor under `timed_walk`): a pick past them was dropped by every caller already, so fewer gates run
  but the answer is the same. The 60k and 100k maps are under `D:/TheCity/target/perf_maps/` (`isle_60000.txt`,
  `isle_100000.txt`, `--downsample 2`, the k2 lines). Left hot at 60k: `stat_daily` 0.53 s in one tick (~0.9 s
  projected at 100k, against the gate's 1 s), the rest of think (`Shop`, `local()`: phase 4).
- **Phase 3** (`OPEN-184` annotated). The world build at 100k seed 42: 405 s -> 54 s (60k: 277 s -> 30 s, `prof`'s
  `world_new`), under the gate's 2 minutes. One new primitive, `transit::reach_discs(to, limit)`: discs whose union
  holds every door within `limit` of `to` by `commute_ticks` (the walk's disc; with the El, around each stop the walk
  left after the half headway, the cheapest hop to one of `to`'s alight candidates and its walk), queried on a
  door grid and cut by the exact measure. Its callers: relocation's pick and trade (`life::HomeReach`, per pass and
  work tile, the Homes within `max(cap, dest_cap)` nearest first: the pick's minimum is order-free and every Home
  past `dest_cap` was skipped; the trade's walk stops past `cap`), `free_beds_near` (the pre-roll's unmet demand and
  `grow_housing`), `build_near`'s Lot and storey (`World::kind_grid`), and the graded pick
  (`classpay::GradedIndex`, citizens by `set_out` once a job-search pass; with nobody within the job's cap, the
  nearest over a doubling reach, every citizen once it spans the map). The labour pool's walk keeps its visits (the
  age refusals it meets are `band_refusals`, a saved, reported counter) and reads each group's rules a hire cannot
  change once, when the role's groups are built (the heirs, lawfulness, dress, the knowledge floor, a chip post,
  the struck-off, the age gate), the hired and quit-blocked by member index. **Deviations:** the study's
  "`seed_staff` from the `HirePool`" was already so (`seed_staff` ends in `job_search`); its 80 s was the pool's
  walk, which visits every group within 512 key units of the best (`profession_w` 400 puts the floor that low), so
  it is cheaper visits, not fewer. Kept current: nothing outlives a pass. `HomeReach` and the door grids are built
  per relocation pass or round (no Home is added inside one), `kind_grid` is rebuilt after a Lot is built on
  (`kinds_gen`), the pool's `member_hired` is set by `note_hired`, and `GradedIndex` holds only where a citizen sets
  out from, which a hire does not move; every other rule is read at the pick. Identical to `2ced00a` on 2k 42-44 x 30,
  20k `isle_m_k2` 42 x 30 (save, report, events) and the 100k day-0 save; `test_indexed_build_matches_the_scans`
  builds the shipped and the 4,000 city both ways (it fails if the pool's hired go stale). At 60k (s, base ->
  branch): `preroll_housing` 172.7 -> 8.6 (`reloc::pick` 110.7 -> 0.13, `preroll::unmet` 20.8 -> 0.24,
  `build_near` 12.2 -> 0.09), `seed_staff` 74.5 -> 6.4 (the pool's walk 59.9 -> 0.28), `pick_graded` 36.0 -> 3.2,
  `seed_after_staff` 15.8 -> 1.0. SAVE_VERSION stays 67. Left at 100k (54 s): `seed_staff` 13 s (the pool's
  per-door `bases`, 3.4 s, and `pick_graded` 8.9 s), the other seeds' own scans (`seed_sellers` 7.5, `seed_venues`
  5.8, `ownership::seed` 5.1, `seed_fixers` 4.2, `ages::seed` 4.2), relocation's per-pass worker scan (23 passes, 5.3 s
  with the trade).
- **Phase 4** (`OPEN-184` annotated). The price-keyed caches, measured first (a scoped profile at 60k) and cut where
  the time was. (1) `spatial::NearTable` (`World::near_table(kind)`, keyed `(building_gen, kinds_gen)` like
  `kind_grid`): per 16-tile bucket the 16 doors nearest its box, ascending `(box distance, id)`, and the box
  distance of the first left out. `World::market_by_price` reads its bucket nearest first and stops once `10 x box
  distance` loses to the best stocked Market (a price is never below 0, so that bounds the cost); the scan runs only
  when the list runs out first. Stock, closures and prices are read at query time, so the table is geometry only.
  `assets::nearest_seller` tests its bucket in the scan's `(door distance, id)` order up to the first pass
  (`NearTable::nearest_passing`), the rest sorted after. (2) `seller_open` and `fixes::staff_present` read the
  building's staff (`staff_of`) with a role test, not every holder of the role: the same set. That, not the scan of
  sellers, was Shop's cost: `seller_open` ran on every seller and each walked the role's workers. (3)
  `assets::lowest_level` (per seller kind) and `World::mean_price` are memoised on `(shop_gen, kinds_gen)`:
  `shop_gen` is bumped by every write path of the Building, Corp and Market stores (`comp_mut`, `insert`, `remove`,
  `despawn`), which covers an owner, a corp's level or a price changing, at midnight or mid-day (tests write prices
  directly). Within a tick's think nothing writes those stores, so the memo holds for the slice. (4)
  `World.res_partners`, the Partner reservations by the agent held (`social::LazyReserved` read every reservation
  in the city per Socialise and Court score), kept by `reserve`, `release_all`, `sweep_reservations` and
  `remove_agent`, rebuilt on load with the per-building index. Identical to `218ca03` on 2k 42-44 x 30 and 20k
  `isle_m_k2` 42 x 30 (save, report, events). At 60k seed 42 (ms a game day, base -> branch): `goal::Shop` 3,541 ->
  928, `goal::Socialise` 1,107 -> 714, `goal::Court` 785 -> 637, `goal::Eat` 566 -> 516, `goal::Earn` 428 -> 226,
  `stat::stat_work` 2,810 -> 1,861, `tick::think` 12,727 -> 8,947, `tick::lod` 5,628 -> 4,162, the tick total
  27,525 -> 21,695 (-21 %); ABAB (print only) 20k 321 -> 355, 60k 56 -> 73 ticks/s. SAVE_VERSION stays 67 (nothing
  saved: every cache is rebuilt on its first query, the partner index on load). **Deviations:** "run twice per think"
  was already fixed (the M13 review: `think_with_offer` scores Shop from one `shop_choice` and the planner reuses
  it), so there was nothing to reuse. Court's occupant sum is not a counter: it measured 0.04 s a game day at 60k
  (101k calls), and the sum only picks the faster of two candidate walks that give the same answer; a counter on
  every occupancy path (deaths, arrests, re-kinds, the Statistical tier) would risk a silent drift for nothing.
  The Bar nearest (`nearest_of_kind` behind `local()`) is not tabled either: 0.08 s a game day at 60k. Socialise
  and Court were not `local()`'s: their time was `best_colocated_partner_in` (the reservation scan, now the index;
  what is left is the walk over a crowded building's occupants).
- **Phase 5** (`OPEN-184` annotated). (1) `World::claimed_by_other(guard, s)` behind `law::chaseable`'s claim test
  (Patrol's `any_located_suspect`, Arrest's and the plan bind's `located_suspects`): a `ClaimIndex` (per guard its
  `[escort, Arrest target]`, per suspect its claimers) kept by marks as `PosIndex` is, not `life::claimed_suspects`'
  walk of every guard per scoring call (226k walks in two days at 60k, 12.7 us each). Kept current: `comp_mut`,
  `insert` and `remove` of a filed guard's Brain, and `index_job` / `unindex_job` (a Job inserted, removed or
  re-posted, a despawn) mark the entity; each read re-files the marked first, so an Arrest plan installed or an
  escort begun mid-tick is seen by the next read; `rebuild_tiers_and_roles` drops it; not saved. (2) `needs`' guard
  test reads a per-tick `TileGrid` of the guards' tiles (`sight`-tile cells, `TileGrid::any_near_with`) instead of the
  row-sorted list's band (every guard within `sight` rows, map-wide). Both answer an order-free yes/no by the same
  rule. (3) The longest tick, measured: the CLI report's `max_tick_ms` and `max_tick_at` (the day's longest
  `citysim::tick` wall and its tick of day) after `ticks_per_sec`, CLI-only (not in `DailyStats`, not saved);
  `phase_measure.py` skips them with `ticks_per_sec` (`WALL_COLS`), `pop_scale.py` prints them per day and run.
  (4) The midnight pass's cheap fix: `demography::hire_base` reads the commute once for its three uses (it was read
  up to three times per candidate tile and door). Identical to `4f021de` on 2k 42-44 x 30 and 20k `isle_m_k2` 42 x 30
  (save, report, events). At 60k seed 42 (ms a game day, base -> branch, one alternated pair): `goal::Patrol` 933 ->
  149, `goal::Arrest` 692 -> 68, `tick::needs` 1,859 -> 1,573, `tick::think` 9,027 -> 7,794, `pool::bases` 349 ->
  162, the tick total 21,910 -> 20,835; ABAB (print only) 60k 72 -> 78, 100k 29.5 -> 36.5 ticks/s. **The longest
  tick is midnight**, not 21:00, every day at both sizes (non-prof, days 1-2): 60k 1.28-1.32 s -> 1.07-1.11 s
  (21:00 0.45-0.51 s); 100k 2.45-2.54 s -> 1.94-2.07 s (21:00 0.79-0.86 s). The gate's 1 s misses at 100k by 2x.
  Midnight at 100k (one profiled tick, day 1): `job_search` 0.65 s (`pick_candidate` 0.39 with `pool::bases` 0.22
  and `pool::groups` 0.09, `pick_graded` 0.22), `living`'s midnight pass 0.37 (no sites inside yet),
  `corp_brain`'s daily (rescore and act) 0.36, relocation 0.14, `word` 0.13, `household` 0.10, then under 0.07 each. Day 0's
  first tick is 2.3 s at 60k and 4.0 s at 100k: the first midnight plus every lazy index built on its first query
  (a loaded save pays it too). SAVE_VERSION stays 67. **Deviations:** item 4 (`nearest_seller`'s night fallback)
  left: with nothing open every seller is tested whatever the order, so a grid saves only the sort. The claim
  index is kept by marks, not a per-tick memo keyed on a counter: a guard's Brain is written every tick he acts,
  so a counter would rebuild per guard think; the marks re-file only the guards written since the last read.
- **Checkpoint** (2026-10-25, main `e5048d7` against the pass base `3728aca`, seed 42, quiet box, `pop_scale.py`
  ABAB, 2 reps, 4 days; maps under `target/perf_maps/`, the 20k one the committed `isle_m_k2`). Days 2-4 median
  ticks/s: 20k 261 -> 387, 60k 33 -> 80, 100k 11 -> 37. Build: 20k 46 -> 6 s, 60k 280 -> 29 s, 100k 370 -> 52 s.
  Longest tick (main only; the base has no column): always midnight; 100k 1.9-2.1 s on days 2-3, 2.6-3.0 s on day 4.
  **A 4-day bench hides a slope.** Main alone, 30 days: 100k ticks/s 44 (d1) -> 18 (d27-30), midnight 1.9 s -> 7.9 s,
  peak working set 1.1 -> 3.1 GB; 20k ticks/s 387 -> ~210, midnight 0.27 -> 0.65 s. Something accumulates with the
  days and is scanned, worse with N (the base slid 15 -> 11 in 4 days too, so it predates the pass). Report columns
  that grow at 100k: `holes_open` 200 -> 2,742, `contracts_open` 29 -> 623, `grudges` 6 -> 385; employed rose
  43k -> 56k and homeless fell 8.1k -> 2.9k over the month, so part of it is a busier city. POPULATION_STUDY's curve
  is a day-4 snapshot. The gate reads days 25-30 from here on, not days 2-4.
- **Phase 6** (`OPEN-184` annotated, `OPEN-203` closed by its column, `OPEN-209`-`211` opened). The slope, diagnosed at
  20k seed 42 x 30 with a per-day profile and store sizes, and read at 100k (a 9-day profiled run, then the branch
  x 30). **What grows** (20k, day 2 -> 10 -> 20 -> 30): social edges 101k -> 328k -> 567k -> 767k (Friends 55k ->
  250k, weak Acquaintances 15k -> 245k), a Full agent's neighbours 12 -> 41 -> 89 -> 144, the co-work pair-hours a day
  0.45M -> 1.10M -> 1.42M -> 1.44M (the Recycler's Sanitation crew 139 -> 562 staff), the walked neighbours a day
  2.1M -> 12.2M; holes 37 -> 556, contracts 94 -> 596, grudges 0 -> 907, memories capped (Full agents under 50), the
  event ring capped at 50k. At 100k: edges 0.40M -> 8.7M by day 30, co-work pairs 1.4M -> ~25M a day from day 10.
  **What scanned it:** the co-work bond (`lod` `work::coworkers`, pairs by the square of a same-role crew, crew cap 0:
  100k 0.75 -> 10.2 s a game day by day 9); every neighbour walk that keeps a few kinds (the midnight's
  `household::visits_due`, relocation's kin test through `children_of_agent`, the gang recruit per think); the gang
  order's inputs, a scan of every citizen per call, run each tick of an order dwell after a Job settles (`OPEN-211`).
  Holes, contracts and grudges are read daily, under 0.01 s a game day. The rest of the 20k slope (+1.0 s a game day
  of goal scoring) is spread over every scorer with no one site; the Statistical rolls did not grow, and gang members
  (0 -> 279), the employed (+12 %) and owned assets (0.3k -> 3k) did: a busier city, not proven. **Fixed,
  byte-identical:** (1) `World::kin_candidates`, a superset of each agent's Spouse, Family and Parent edges filed by
  `World::note_kin` where an edge is made kin (`set_spouse`, a birth, an immigrant child, `ages`' grown child, an
  exile's edge, an ambient household), unfiled with the edge, rebuilt exact on load (not saved); `kin_of`,
  `children_of_agent`, the siblings at a birth, the spouse heir, `grieve` and the healer's payers read it and filter
  by kind as before, so the same agents in the same order. (2) The gang recruit walks the open rosters when they are
  shorter than the agent's edges (the best is a total order on affinity, gang and roster position). (3) The order
  inputs read `residents_of`. (4) The CLI report's `edges`, `edges_new` and `coworker_pairs` (CLI-only and not saved,
  after `max_tick_at`; the two counters count from a load). Identical to `7ad2ec7` on 2k 42-44 x 30 and 20k `isle_m_k2`
  42 x 30 (save, report, events). 20k (one pair, base -> branch, print only): ticks/s days 2-4 404/383/368 ->
  398/386/365, days 27-30 185/218/190/205 -> 177/217/192/198; midnight days 2-4 270/259/285 -> 266/244/277 ms, days
  27-30 640/586/678/628 -> 501/433/529/457 ms (`household` 0.19 -> 0.01 s, `reloc::loop` 0.09 -> 0.01 s at day 28).
  100k seed 42 x 30 (the branch against the checkpoint's base run): ticks/s days 2-4 41/38/35 -> 41/38/35, days 27-30
  16/20/18/18 -> 18/21/19/19; midnight days 2-4 1.9/2.0/2.6 s -> 1.8/1.9/2.4 s, days 25-30 7.5-8.1 s -> 3.8-4.1 s;
  peak working set 3.06 GB both. **Deviations:** the per-tick slope is growth by design, so it stays: the co-work
  bond's square (`OPEN-209`; a Sunday, with no shifts, runs ~10 % faster at 100k day 28) and the acquaintance graph's
  month-long prune lag (`OPEN-210`; 8.7M edges, the working set). Both are behaviour (who befriends whom) for Dylan,
  with `OPEN-205`. The Friend-filtered walks (`faith`'s spread, the visit hosts, `leisure`'s contacts, Court's
  candidates) and the debt walks are left: Friends are the growing set itself, and each is under 0.05 s a game day at
  20k. `OPEN-211` (the dwell's per-tick rescore) is a behaviour fix (`rep_flips`, `order_trace`). Three tests that
  set a kin kind by hand file it (`note_kin`); no new test (R7). SAVE_VERSION stays 67. Midnight at 100k is still
  3.8-4.5 s: phase 7's.
- **Phase 7** (`OPEN-184` annotated). Midnight, measured per pass before anything was touched: `run --prof-midnight`
  (with `prof`; each day's midnight tick alone under `PROFDAY <day>`), with PROF sites added inside `living`'s
  (`mid::*`), `word`'s, `ownership`'s (`own::*`), `corp_brain`'s (`corp::*`, `act::*`, `gi::*`), the gangs'
  (`gang::*`), `districts`' (`dist::*`), `faith`'s and `dollhouse`'s midnight passes, so the day-28 midnight is
  accounted for site by site. **What doubled it** from day 1 to day 28 at 100k (seed 42, profiled, the midnight's
  `tick::` sum): days 2-4 2.36 s, days 27-30 4.33 s; `faith::daily` 0.09 -> 1.37 s (its spread walks every
  follower's neighbours: followers uncapped by decision and the graph growing, `OPEN-210`), the gang rescore 0.01
  -> 0.34 s, `ownership`'s rehouse 0.00 -> 0.22 s, `word`'s reputation 0.10 -> 0.21 s, `social::daily` 0.01 -> 0.16 s
  (every edge), the jail's upkeep and the held 0.01 -> 0.15 s; `job_search` fell 0.57 -> 0.23 s and `corp_brain`
  0.71 -> 0.24 s with the days (days 3-4's Grow refits, ~78 a night, each a full `districts::rebuild` of ~6 ms at
  100k). At 20k the same passes, a fifth the size (days 27-30 0.44 s). **Fixed, byte-identical:** (1) `faith`'s
  spread and Preach test their own pure draw (`p_spread` 0.0005-0.001, `p_attend` 0.3) before the edge or the agent,
  the same converts in the same order. (2) `districts::rebuild`'s grid half (the district and street byte per tile,
  the masks, the walk tiles, streets and centroids) is memoised on `World::district_geo` (not saved, empty in a clone)
  under its whole input compared as data, not hashed: the map's size, `Map::tile_edits` (bumped by `set_tile`), the
  rubble, the rows, every standing rect in store order; a hit on kept rows rewrites only the per-building lists
  (nothing else writes the grid half). (3) `ownership::rehouse` reads each Home's rent once a pass (nothing in the pass
  moves a rent) and an adult who can pay no Home skips the walk. (4) The gang frontier and `chrome::harvest_target`
  test the side or ground before the residents and the reputation. Identical to `7359e73` on 2k 42-44 x 30 and 20k
  `isle_m_k2` 42 x 30 (save, report, events). Midnight at day 28, profiled (`tick::` sum, base -> branch): 20k 384
  -> 360 ms (`faith` 26 -> 9, rehouse 15 -> 2); 100k 4.07 -> 2.67 s, days 27-30 4.33 -> 2.63 s (`faith` 1.37 ->
  0.21, rehouse 0.22 -> 0.01, `ownership` 0.29 -> 0.07, the gang rescore 0.34 -> 0.29). Non-profiled, one pair
  (print only), 20k midnight days 2-4 251/237/261 -> 240/232/252 ms, days 25-30 387/420/453/386/484/423 ->
  353/396/368/352/430/379 ms. 100k seed 42 x 30 (the branch against phase 6's `long100.txt`, main): midnight days
  2-4 1.82/1.91/2.36 -> 1.66/1.66/1.70 s, **days 25-30 3.85-4.15 -> 2.59-2.71 s**, the worst day 4.53 -> 3.24 s
  (day 14); ticks/s days 27-30 18/21/19/19 -> 18/22/20/20; peak working set 3.06 GB both; the build 57 -> 47 s
  (print only). **The gate's 1 s is missed by 2.6x**: what is left at day 28 is a long tail (100k, branch, days
  27-30): the gang rescore 0.29 s (149 gangs; `harvest_target` 0.20 s, a scan of every Kit for each scav gang),
  `faith` 0.21 s (spread 0.17), `job_search` 0.19 s, `word`'s reputation 0.17 s, `social::daily` 0.15 s (8.7M
  edges), relocation 0.15 s (its worker scan 0.11 s), `corp_brain` 0.21 s, the law's midnight 0.21 s (jail upkeep
  0.09, the held 0.06), `contracts` 0.10 s, `household` 0.09 s, the corpses 0.09 s, then ~40 passes under 0.07 s
  each. No pass is over 0.3 s, so a stagger of the passes over the night's ticks would bring the longest tick to
  ~0.35 s (a 100k tick is ~50 ms on days 27-30, plus the largest pass); a parallel split would need ~3x on the 2.6 s.
  Both are design questions for Dylan. The 60-day read for `OPEN-210` (main, 20k `isle_m_k2` seed 42; `edges`,
  `edges_new` at days 10/20/30/40/50/60): 328k/36k, 567k/19k, 767k/22k, 869k/21k, 960k/23k, 1,036k/21k; the
  month's prune slows the net growth (+22k a day on days 10-30, +10k on 30-40, +9k on 40-50, +8k on 50-60) but has not
  levelled it by day 60; midnight there 0.51-0.53 s on days 50-60. SAVE_VERSION stays 67. **Deviations:** no
  stagger and no parallel split (out of scope); `job_search`'s pool and graded picks were not cut: on days 25-30
  they are 0.19 s at 100k, below five other passes, where phase 5's day-1 profile had them first. The Friend walks
  (`faith`'s spread) stay a walk of the growing set (phase 6's finding, `OPEN-210`); the held's meeting draws one
  word per stranger per arrival, so it is not prunable. No new test: the memo is built every call under
  `spatial::scans`, so phase 3's `test_indexed_build_matches_the_scans` covers it (R7).
