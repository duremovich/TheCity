# Population study: how far the island city goes (OPEN-184)

Dylan 2026-10-23: "Let's see how far we can push population, with an eye towards future optimization. Better to have a
full metropolis if we can swing it." Measured 2026-10-23 on main `60bc307`, seed 42, release, single thread, on a
shared box (another agent's sims ran alongside; see § Noise). The bench is `tools/pop_scale.py`; its logs and the
per-size maps stayed in the session scratchpad.

## Answer

- **Today: 60,000 for the game city.** It runs 31 ticks/s, which is 44× the view's walk-matched X1 (0.7 ticks/s)
  and clears 30× fast-forward (21 ticks/s) with ~10 ticks/s to spare for the view and the camera ring. 80k runs
  15.5 ticks/s, under the 20 ticks/s fast-forward line. 100k runs 9.5.
- **After roadmap items 1-5 (all index or cache work, byte-identical): 100,000.** Those items remove ~60 % of the 80k
  day and most of the N² term. The projection is ~40 ticks/s at 80k and ~25 at 100k.
- **Past ~120k the import binds, not the CPU.** A `k = 2` island has too few cuts for the employers, so 150k is short
  of Farms and Lots. It needs a `k = 1` import (§ Capacity).
- **The cost is quadratic in N, not linear.** Ticks per second fall ~N^-2.0 between 20k and 100k. The bodies scale
  per capita (`Config::scaled_to`: Full 2.5 %, Coarse 7.5 %), and each body's think scans city-wide building lists
  that also scale per capita (venues, Markets, sellers, guards). That N × N is the whole story; the rest is ~N^1.5
  (cache misses on linear work).
- **Fix the 21:00 stall before any view work, at any size.** `leisure::stat_daily` runs once a day in one tick. It
  takes 0.55 s at 20k, 2.9 s at 40k, 7.9 s at 60k and 17 s at 80k. At X1 that freezes the view for 6 game-minutes at
  60k.

## The curve

`python tools/pop_scale.py --bin plain=<release exe> --map "<scratch>/maps/isle_{n}.txt" --pop 20000,...,150000
--days 4 --reps 2 --save`, with the default `--ambient-flats 50`. Each size runs on its own re-import (`import-graphics
--downsample 2 --residents N`, § Capacity), because the committed `isle_m_k2` holds only 31,645 (§ Capacity). The 20k
re-import is byte-identical to the committed map.

- **ticks/s** is the median of the steady days (days 2-4), with each repetition's own median in brackets.
- **build** is the wall before day 1 (`World::new`).
- **peak MB** is the working set: the first figure is without a save (the profiled runs), the second is with the end
  save written.
- The tiers and counters are from day 4 of the last repetition.

| residents | ticks/s (reps) | ms a tick | wall a game day | build | peak MB (no save / save) | save | Full / Coarse / Statistical | employed | homeless |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 20,000 | 245 (223, 267) | 4.1 | 6 s | 46 s | 292 / 416 | 73 MB | 500 / 1,500 / 17,990 | 51 % | 263 |
| 40,000 | 60 (60, 60) | 16.7 | 24 s | 169 s | 478 / 728 | 144 MB | 1,000 / 3,005 / 36,038 | 50 % | 722 |
| 60,000 | 31 (32, 30) | 32 | 47 s | 294 s | 688 / 919 | 212 MB | 1,499 / 4,499 / 54,015 | 49 % | 1,234 |
| 80,000 | 15.5 (16, 15) | 65 | 93 s | 426 s | 809 / 1,403 | 281 MB | 1,999 / 6,001 / 72,017 | 47 % | 1,638 |
| 100,000 | 9.5 (9, 10) | 105 | 152 s | 486 s | 1,087 / 1,603 | 353 MB | 2,500 / 7,504 / 90,009 | 45 % | 2,187 |
| 150,000 | 8 (8, 8) | 125 | 180 s | 508 s | 1,422 / 2,616 | 528 MB | 3,749 / 11,254 / 134,803 | 37 % | 4,827 |

- **150k is a different city.** Its import is short of Farms (694 of 900) and Lots (1,330 of 12,000), so it has fewer
  jobs, and its leisure cost falls (Unwind 37 s → 9 s a day, the 21:00 pass 27 s → 4 s), likely from fewer venues on
  the short Lots (not counted). That is why it is barely slower than 100k. Do not read
  it as the curve flattening.
- **Ambient households fall as N rises:** 714,635 at 20k, 500,532 at 100k. More of the import's cuts become employers,
  and more of each Home's flats become slots. Nothing is materialised in 4 days.
- **Memory and saves are linear and cheap.**
  - The working set is ~10-14 KB a resident without a save.
  - A save is 3.6 KB a resident. Writing it costs about 1× the working set again, transiently.
  - Neither binds below 1M residents on this 63 GB box.
- **The tiers are scaled, not fixed.** `scaled_to` multiplies `[lod] max_full` 50 and `max_coarse` 150 by N / 2,000,
  so bodies are 10 % of the city at every size, and the forced quotas (`private_quota`) and the planner and path
  budgets scale with them.
  - OPEN-149's ring snapshot found ~150 Full free at 20k against a `max_full` of 500, because the forced classes
    (gang, guards, raids) take the rest. This study did not re-measure that.
- **`--ambient-flats 50` does nothing on the island.** Every map Home carries `flats=`, and `ambient::ambient_flats`
  reads `flats=` first; `flats_per_slot` applies only to a map without it.

## Capacity: what binds

- **The committed `isle_m_k2.txt` houses 31,645.**
  - Its 4,280 Homes carry 6,329 floor slots, and `World::new` deals 5 residents a slot (`residents_per_home`).
  - A resident past that starts homeless. A 1-day probe on the committed map at 100k had 66,810 homeless.
  - Nothing panics or asserts up to 150k. The first thing that breaks is beds, then jobs.
- **The slots are dealt by `import-graphics --residents N`, not by the island's flats.**
  - The importer deals N / 5 households over the Homes by their flats' share, and sizes the employers per 1,000
    residents.
  - Re-importing for N gives these, from the same Oct 12 export of `TheCityGraphics/assets/city/isle_m/out`:

| `--residents` | Homes (slots → beds) | Home flats | Farms | Markets / Bars | Lots (wanted) | short |
| --- | --- | --- | --- | --- | --- | --- |
| 20,000 | 4,280 (6,329 → 31,645) | 782,702 | 120 | 30 / 30 | 1,600 (1,600) | none |
| 40,000 | 4,043 (10,109 → 50,545) | 731,256 | 240 | 60 / 60 | 1,617 (3,200) | Lots |
| 60,000 | 3,886 (13,661 → 68,305) | 690,662 | 360 | 90 / 90 | 1,554 (4,800) | Lots |
| 80,000 | 3,729 (17,156 → 85,780) | 636,066 | 480 | 120 / 120 | 1,491 (6,400) | Lots |
| 100,000 | 3,572 (20,566 → 102,830) | 565,232 | 600 | 150 / 150 | 1,428 (8,000) | Lots |
| 150,000 | 3,326 (30,013 → 150,065) | 499,994 | 694 (900) | 225 / 225 | 1,330 (12,000) | Farms, Lots |

- **Cuts bind first, from 40k: Lots.**
  - The `k = 2` import has 6,104 cuts. Each employer takes a cut away from housing.
  - The Lots (the founding and venue sites) stop at ~1,500 against 80 per 1,000 wanted. The importer will not take
    Blocks past the generator's 80 Lots to 200 Blocks.
  - With fewer Lots, there are fewer founding and venue sites per capita: 14 Lots per 1,000 at 100k against 80 at 20k.
    That is a calibration drift (employment 51 % → 45 %), not a crash; the venue count per capita was not measured.
- **Farms bind at ~120k.** At 150k there are 694 of the 900 wanted.
- **Hard caps that do not bind:**
  - A Home's slots are `u8`; 0 were capped at 255 even at 150k.
  - The `u16` levers clamp at 65,535.
  - The working set is ~1.4 GB at 150k.
- **The P9 re-import (OPEN-182)** drops the island's flats from 843,847 to 335,675.
  - Households are dealt from `--residents`, so 335k flats still hold 100k residents' 20,000 slots with ~15× to spare.
    The P9 flats move only the Ambient count (~300k households) and which towers get the slots.
  - What a bigger city needs from the re-import is cuts, not flats. P9's 2,858 new `shop_house` lots, and a venue on
    its own lot (`B ... lot=<id>`, OPEN-182), give the employers and Lots room.
  - Past 100k it needs `--downsample 1` (14,516 lots, about 2× the cuts).

## Where the time goes

Profiled one run a size (`--features prof`, 3 days, `--prof-from 1`, so days 1-2 are profiled). The figures are wall
ms a game day. The exponent is the log-log slope from 20k to 100k (150k is excluded, see above). Runs without the
profiler track the profiled ones within 10 %.

| site | 20k | 40k | 60k | 80k | 100k | exponent | what scales |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `tick::think` (all goal scoring) | 2,233 | 9,241 | 21,127 | 47,480 | 72,068 | **2.18** | bodies × city-wide lists |
| └ `goal::Unwind` | 700 | 3,590 | 9,581 | 23,290 | 37,410 | **2.50** | `leisure::nearest_venue` collects and sorts every venue of each of 11 kinds per call |
| └ `goal::Shop` | 200 | 1,340 | 3,162 | 7,610 | 11,901 | **2.54** | `assets::shop_choice`: up to 6 full seller scans (`lowest`, `seller_level` → `corp_of_building`), run twice per think (`already_satisfied` repeats it) |
| └ `goal::Patrol` + `goal::Arrest` | 148 | 782 | 2,021 | 4,604 | 7,598 | **2.47** | `life::claimed_suspects` walks every guard per call |
| └ `goal::Socialise`, `Court`, `Earn`, `Eat` | 469 | 1,586 | 3,254 | 6,722 | 9,023 | 1.9 | `World::local` → `nearest_of_kind` / `market_by_price` scans every Market or Bar; Court sums occupants over every Bar and Market |
| `tick::living` = `leisure::stat_daily` | 553 | 2,852 | 7,928 | 17,448 | 26,822 | **2.45** | 21:00, one tick: every Statistical agent × 11 kinds × `nearest_venue` |
| `tick::lod` (`run_statistical`) | 1,205 | 3,313 | 5,840 | 9,827 | 12,396 | 1.47 | the Statistical hour; `stat_work` 1.62 (co-worker pairs, `[lod] coworker_crew` 0), `stat_earn` 2.02 |
| `tick::exec` | 724 | 2,127 | 3,640 | 5,935 | 6,805 | 1.43 | bodies; `exec::big_path` 1.15 |
| `tick::plan` | 249 | 894 | 2,087 | 4,688 | 7,090 | 2.11 | bodies × plan-time scoring (Unwind's spot scoring, the same venue lists) |
| `tick::demography` (daily) | 444 | 1,327 | 2,328 | 3,932 | 4,688 | 1.50 | `job_search`: `pick_candidate` 1.56, `pick_graded` 2.42 |
| `tick::needs` | 356 | 1,316 | 2,224 | 3,498 | 3,942 | 1.52 | bodies, plus a per-tick sort of every guard's tile (`guard_tiles`) |
| `tick::law`, `social`, `mood` | 300 | 826 | 1,285 | 2,005 | 2,400 | 1.3 | linear-ish |

- **The ranking at 80k** (94 s a game day):
  - think 50 %: Unwind 25 %, Shop 8 %, Patrol and Arrest 5 %, the `local()` scorers 7 %;
  - the 21:00 stall 18 %;
  - the Statistical hour 10 %;
  - exec 6 %, plan 5 %, demography 4 %, needs 4 %.
- **Statistical work that is O(1) a call** (for example `stat::stat_rolls`) still grows N^1.6. That is the memory
  wall: a working set of 0.3 GB at 20k and 1.1 GB at 100k, with `Vec<Option<T>>` stores indexed by entity. It is the
  floor that any index work leaves, and the target of the data-layout items.
- **World build is a quadratic of its own.** `--days 0` at 60k takes 262 s:
  - `life::preroll_housing` 140 s, of which `reloc::pick` is 87 s over 578k calls: movers × every Home;
  - `wages::seed_staff` 91 s, of which `job::pick_candidate` is 80 s over 29,756 vacancies. That is the pre-S1
    per-vacancy `world.citizens()` scan, still used at seeding, at 2.7 ms a vacancy;
  - `job::pick_graded` 39 s;
  - `classpay::seed_after_staff` 17.5 s.
- **Daily and hourly spikes are the view's real problem, more than the mean.**
  - The 21:00 `stat_daily` stall, measured above.
  - The midnight passes: demography's `job_search` and relocation, 1.3 s at 40k and 3.9 s at 80k in one tick.
  - The hourly `lod::assign_hour`: 3 ms at 20k, 16 ms at 80k a call, small.

## The Full budget (the camera ring, OPEN-149)

- **Per body.** Think, exec, plan and needs come to 1.24 µs a body a tick at 20k, 2.4 at 40k, 3.4 at 60k, 5.3 at
  80k and 6.2 at 100k. A body's cost grows ~N^1.0, which is the city-wide lists.
- **Per Statistical agent.** It costs 0.046 µs a tick at 20k and 0.095 µs at 80k, so a body costs ~25-55× a
  Statistical agent.
- **Full over Coarse.** OPEN-149's probe (20k gen, `max_full` 50 → 2,000) put a Full agent's extra cost at 1.3 µs a
  tick. This study did not split Full from Coarse, so the next figure is scaled from that probe, not measured.
- **Scaled by the per-body growth**, the marginal Full agent costs ~1.3 µs × N / 20k. The Full agents a ring can add
  and still hold 20 ticks/s (a 50 ms tick) are:

| residents | tick today | headroom at 20 ticks/s | marginal Full | ring Full affordable today | after items 1-5 (projected) |
| --- | --- | --- | --- | --- | --- |
| 20,000 | 4.1 ms | 46 ms | 1.3 µs | all of them | all |
| 40,000 | 16.7 ms | 33 ms | 2.6 µs | ~12,000 | all |
| 60,000 | 32 ms | 18 ms | 3.9 µs | ~4,500 | ~15,000 |
| 80,000 | 65 ms | none | 5.2 µs | 0 (it misses 20 ticks/s) | ~8,000 |
| 100,000 | 105 ms | none | 6.5 µs | 0 | ~2,000 |

- **A ring needs only a few hundred Full**: the busiest 256 m cell holds 33 bodies at 20k, and OPEN-149's ring
  snapshot had 273 agents. Every size up to 60k affords that today, and up to 100k after the roadmap.
- **The binding limit is `max_full` itself, not the CPU.** The ring needs its own budget outside `max_full`, as
  OPEN-149 says.

## Recommended target

**60,000 now; 100,000 once items 1-5 land; past 120k only on a `k = 1` import.**

- **60k** has 3× the street of 20k: the busiest cell goes from ~33 bodies to ~100, at 31 ticks/s. That is 44× X1 and
  clears 30× fast-forward. It affords ~4,500 ring Full at 20 ticks/s.
- **The 60k gate cost.** `core_20k` would become a 60k gate at ~4× the wall. The 2k fixture stays the fast tier
  (OPEN-136).
- **Build time at 60k is 5 minutes**, acceptable for a pre-run starting world (OPEN-150), and item 2 halves it.
- **What 60k needs first:**
  - item 1 (the 21:00 stall);
  - a 60k import, which today is `--residents 60000` on the Oct 12 export (Lots short);
  - the re-baseline of the 20k-tuned FINDINGs at 60k. No knob gets retuned: employment reads 49 % against 51 % at
    20k, and homeless 2.1 % against 1.3 %.
- **100k after the index work.** It is a "full metropolis", but at today's 9.5 ticks/s it misses fast-forward. With
  items 1-5 the projection is ~25 ticks/s, borderline once the view takes its share, so items 6-7 are the margin.
- **Not 150k on `k = 2`.** The import cannot employ it (Farms 694 of 900), and its numbers are a different city.

## Optimisation roadmap (ranked by gain per effort)

Gains are from the 80k profile (94 s a game day). Every item is meant to be byte-identical:
- the same candidate, chosen by the same key, with the same tie-break (door distance, then id), from a superset the
  grid or cache returns;
- proven by the off-switch identity on seeds 42-44 × 30 days (doctrine 6);
- with no new random draw (doctrine 5).

A cache's risk is staleness. Each needs a named invalidation point (a building opened, closed or re-owned, prices at
midnight, a guard hired or killed), or it must be rebuilt at a fixed cadence that the old code's inputs also change
on.

1. **The venue index (`leisure::nearest_venue`).** Fix: a per-kind `spatial::BuildingGrid`, which already exists and
   `vehicles::steal_target` uses. Query nearest-first and stop at the first open venue that passes, in place of
   collect-every-venue-and-sort.
   - **Gain:** `goal::Unwind` 23.3 s + `stat_daily` 17 s + part of plan ≈ **45 % of the 80k day**. It removes the
     21:00 stall, the largest single N² term.
   - **Effort:** small. Venues change only at founding and closing.
2. **World build.**
   - Fix: `seed_staff` uses the `HirePool` that `demography::job_search` already builds, in place of the per-vacancy
     `pick_candidate` scan. `preroll_housing`'s `reloc::pick` and `free_beds_near` query a Home grid by door.
   - **Gain:** ~200 of 262 s at 60k. The build becomes ~linear (~1-2 min at 100k).
   - **Effort:** small to medium.
   - **Risk:** the same hires in the same order. Seeding is part of the save hash, so prove it with a day-0 save
     identity.
3. **`shop_choice`.** Fix: cache the per-kind lowest `seller_level` daily (prices move at midnight). Back
   `nearest_seller` with the grid. Reuse the think's own `shop_choice` result in `already_satisfied` rather than
   computing it twice.
   - **Gain:** ~7 s, **8 %**.
   - **Effort:** small.
4. **`World::local` for Market and Bar.** Fix: a per-tile-bucket or per-district nearest table rebuilt at midnight
   (`market_by_price` reads midnight prices), with Bars from the grid. Court's occupant sum over every Bar and Market
   becomes a counter kept by `enter_building` and `leave_building`.
   - **Gain:** Socialise, Court, Earn and Eat, plus the jobless `stat_work` branch, ≈ 7-8 s, **8 %**.
   - **Effort:** small.
5. **`life::claimed_suspects`.** Fix: one claimed set per tick (memoised like `located_suspects`), or kept on Arrest
   install and escort.
   - **Gain:** Patrol + Arrest 4.6 s, **5 %**.
   - **Effort:** small.
   - **Also:** `needs`' per-tick `guard_tiles` sort becomes a guard TileGrid (~2 %).
6. **The Statistical hour.** Fix: `[lod] coworker_crew` > 0 (the code and its test exist). It bounds the co-worker
   pairs from O(staff²) to O(staff × crew) per employer an hour, and shrinks the edge graph that the degree-walking
   scorers read.
   - **Gain:** `stat_work` + `coworkers` ~6.6 s, **7 %**, and less in think.
   - **Doctrine risk:** this is a behaviour change, not an index. Friend edges fell 43 % at 2k with 32. It needs the
     parity test, a FINDING and Dylan's call.
   - **Effort:** small to code, medium to judge.
7. **Parallel scoring.** Fix: think's goal scoring reads the world and writes only the agent's own brain. Score the
   tick's think slice in parallel (rayon `par_iter` collect into a Vec), then apply the results in id order.
   - The same applies to the Statistical hour's 1/60 slot: two-phase, compute the rolls per agent on its own keyed
     stream (`SimRng::agent`), then apply writes (edges, wallets, holes) in id order.
   - **Gain:** think is 50 % of the tick. ~4-6× on it with 8 cores means **~40 % of the tick**, more once items 1-5
     cut its lists.
   - **Effort:** large. Every scorer's hidden writes (memo caches like `located_suspects`, the plan queue) must move
     to the apply phase.
   - **Doctrine risk:** determinism. Reductions must be ordered and no draw may come from a shared stream, so prove
     it with save-hash identity across thread counts. LOD parity is untouched.
8. **Data layout.** Fix: the hot components (`Position`, `Needs`, `Brain`, `Job`, `Household`) for the Statistical
   tier packed in a tier-ordered SoA, in place of `Vec<Option<T>>` by entity index.
   - **Gain:** the N^1.5 floor of the "linear" work (lod, exec, needs) is ~25 % at 80k, perhaps half of it.
   - **Effort:** large.
9. **Bodies not per capita above the game city's size.** `scaled_to`'s `max_full` and `max_coarse` stay at the target
   city's values, plus the ring's own budget (OPEN-149).
   - **Gain:** linear in the cut.
   - **Doctrine risk:** S4 measured that the gangs die without per-capita bodies (Scale S4). It is a design call,
     listed only so it is not mistaken for an optimisation.

**Projection:**
- Items 1-5 take the 80k day from ~94 s to ~35 s, which is ~40 ticks/s. They take 100k from 152 s to ~60 s, which is
  ~25 ticks/s.
- Item 7 roughly doubles both.
- The remaining slope is ~N^1.4-1.5, the memory floor.

## Noise and what needs a quiet-box re-run

- **This box was shared.** Another agent's sims and one profiler build overlapped some runs. The two repetitions
  agree within 5 % at 40k-150k, and 20k spread 18 % (223 vs 267). One 40k day read 39 against 60 elsewhere.
- **Re-run on a quiet box:**
  - every absolute ticks/s, especially 20k;
  - the build times;
  - the Full-budget table, which is derived, not measured. A direct probe is `max_full` raised on the 60k re-import,
    as OPEN-149's probe at 20k.
- **The exponents and the ranking are robust:** the same order at every size, from one profiled run each.
- **The study's own changes:**
  - It added prof-only timed sites inside `living::run` and `leisure::hourly`. `timed!` expands to its expression
    without the `prof` feature, so the sim is unchanged.
  - The 80k split profile is from a second profiled binary with those sites; its totals match the first within 2 %.
