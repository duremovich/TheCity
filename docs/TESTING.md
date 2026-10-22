# Testing

2026-10-09 (Dylan): "The most important thing is emergent story. For that we need people that appear to act like people." The suite is a fast core that catches a *broken* city, a behaviour tier that judges whether people *act like people*, and a short list of god scenarios. Rewritten 2026-10-18 (the process review) around budgets and caps, because the 2026-10-09 prune regrew within eight days (663 → 1,347 tests, god 13 → 41). The old page with the per-seed history is `docs/history/TESTING_2026-10-09.md`; the per-seed counts also live in the tests' own comments.

**The doctrine, in three lines:**

1. Asserts catch broken: a panic, nondeterminism, a coin made or lost, a collapse, a mechanism that never fires.
2. Behaviour judges alive: the archetypes' diaries and the person probes are the primary judge.
3. Numbers are reports, not gates: `FINDING` lines, read with `tools/analyze_run.py` from `citysim-cli run --report`.

## The tiers

One command each (`python tools/test.py <tier>`); each runs its cargo steps one at a time (the change tier runs its long poles, `core_sanity` and the seven slow binaries' longest tests, in one lane beside its tests step: `CHANGE_BESIDE` in `tools/test.py`, since the island pass phase 0), logs everything under `target/test-logs/`, and prints one line per binary, the FAIL and FINDING lines, and the wall time against the budget.

| tier | command | what | asserts | budget | measured 2026-10-18 (2026-10-19) |
|---|---|---|---|---|---|
| **change** (every change) | `test.py change` | `cargo fmt --check`, `clippy -D warnings`, every non-ignored test: ~1,280 unit and integration tests in `citysim/tests/*.rs`, `citysim-cli`, plus `core_sanity` | each mechanism does what it says; `core_sanity`: seeds 42-44 x 120 days, the coin identity and `escrow_leak` daily, the collapse bounds, the mechanism-existence bullets, ticks/s printed | 4 min (3 until 2026-10-18: set before the measurement) | 4m43s: tests 272 s (`core` 40 s, `wounded` 28 s, `wages` 18 s, `person` 12 s, 80 binaries under 3 s), fmt + clippy 10 s. (2026-10-19, the Lives at work gate: 3m29s with clippy warm; `core` 40-45 s, `person` 14 s, `weapons` 10 s, `wages` 9.5 s, `bodies` 7 s, `ambient` 7 s, `leverage` 6.5 s; paid back from 4m25s by threading tests' independent cities, one built base world a binary (`OnceLock`), no empty doctest pass; the gate close 8c: 3m36s, `core` 41 s, `person` 14 s, `wages` 9 s, `weapons` 10 → 7 s) |
| **merge** (before a merge to main, on the exact tree) | `test.py merge` | `core_year` (seed 42 x 365, the bounds per 30-day window); the behaviour tier (`shadow --assert`: 135 per-archetype bounds on seeds 42-44, calibrated on the noise); the god scenarios (seed 42, 60 days, each against its control) | the city holds a year; people still act like people; the world reacts to a shock | 8 min | year ~100 s, behaviour ~80 s (~3 GB), god ~100 s at 20 scenarios (measured 2026-10-18: `god` 61 s, `god_corps` 18 s, `god_districts` 19 s, the three binaries in series, 108 s wall with an incremental build; the 41 took ~130 s). (2026-10-19: 4m16s; year 115 s, behaviour 58 s, `god` 49 s at 15, `god_corps` 17 s, `god_districts` 17 s; 8c: 4m26s, year 116 s, behaviour 63 s, `god` 50 s) |
| **milestone** (at a close, or nightly) | `test.py milestone` | Full-vs-Statistical parity (`lod.rs`, the 2,000 city, rates within 15 %); `core_20k` (the 20,000 city, the bounds per capita, determinism of two runs); `god_cut_the_precinct` (`god_20k.rs`: two 20k cities, 40 days) | the Statistical tier mirrors Full; the big city holds | 20 min | parity ~3.5 min, 20k ~5 min (2026-10-19: `god_20k` 392 s, two 20k cities x 40 days; the tier read 7m51s before it; 8c: 13m46s, parity 235 s, `core_20k` 244 s, `god_20k` 347 s) |
| **diag** (on demand) | `test.py diag --only NAME` | print-only probes (`hit_probe`, `law_probe`, `class_table`, the censuses, `measure_*`); the lives baseline is `citysim-cli shadow --lives [--population 20000]` | nothing | — | listed by `test.py diag` |

Plus the human judge at every milestone: a shadow pass (`citysim-cli shadow --assert --out shadow_out`, critics read one diary per archetype, `docs/SHADOW_V4.md` is the pattern). The bounds catch regressions between reads; the read is the verdict on "alive".

**Budgets** are wall time after compile with the tier alone on this box. A breach prints loudly and never fails (the box is noisy, Blender renders on it): **the phase that breached pays it back before it closes** (retire, merge into a shared run, or speed up). The numbers are `BUDGETS` in `tools/test.py`; change one only with a line in the commit message. `core_sanity`'s ticks/s floor is read only from a run alone; perf regressions are judged by alternating A/B of two binaries.

## The caps (`test.py lint`)

- **God scenarios ≤ 20** plus controls, one per reaction class (below). A phase's scenario is a spec-time device: it runs at the phase's acceptance and at the close it replaces its class's scenario or is deleted, write-up to `docs/history/`.
- **Asserted `core_sanity` bullets ≤ 60** (`mechanisms()` + `jobs2_mechanisms()` + `transit_mechanisms()` in `core.rs`). A new bullet replaces one or names the milestone that earns a row. Printed lists (`printed_mechanisms`, `jobs2_printed`) are uncapped: they cost nothing.
- **Every `#[ignore]` has a reason**: `tier: merge`, `tier: milestone`, `tier: diag`, or `gap: OPEN-nnn` (an open row in `docs/OPEN.md`). A `gap:` older than two milestones is fixed or deleted at the next close.
- **Test files are named by system**, never by the pass that wrote them; a file matching `_review|_fixes|shadow_v|_probe` may hold only `tier: diag` tests.
- A test file over 1,500 lines is a warning: split by system.

`test.py report` prints the close-of-milestone table (tiers against budgets, counts against caps, ignores by tier, test files and lines, helper copies outside `tests/common`); `/close-milestone` pastes it into HANDOFF and every cap over becomes an `OPEN` row owned by the next milestone.

## Adding a test

**A mechanism** gets a unit test in a seeded world: set the preconditions by hand, call the system, assert the effect (`tech.rs::test_research_held_builds_a_lab_in_the_focus`, `camp.rs::test_unfed_child_taken_at_take_days_before_death` are the pattern). Build the world with `tests/common` (`world()` the shipped seed-42 city, `dole_world()` the pre-flip city most unit tests pin, `adults`, `set_coins`, `coins`, `corp_named`, `run_days`, `identity`). Most unit files run `Config::load().dole_city()` so they pin the mechanisms they were written for; `core.rs`, `lod.rs`, `person.rs`, `policy.rs`, the god scenarios and the behaviour tier run the shipped default.

**A `core_sanity` bullet** only when the mechanism fired on ≥ 3 of 6 seeds (42-47 x 120 days): run `core_sanity --nocapture` (it prints every event kind and mark per seed), add `("a Thing", k(EventKind::Thing))` or a mark with the 42-44 counts in a comment, and retire a bullet or justify the row. A bullet that turns rare is demoted to `printed_mechanisms` with its unit test named beside it. Never add a band.

**A person probe** (`person.rs`): the shipped seed-42 city, one agent pinned at Full, one stimulus through its components, `run_to`, and an assert on what a person would do, read from the agent's own state. A probe that exposes a gap is `#[ignore = "gap: OPEN-nnn"]`; the fix is a behaviour change, not a test change.

**A god scenario** (`god.rs`, `god_corps.rs`, `god_districts.rs`): seed 42, 60 days, the shock at day 10-45, against the shared `OnceLock` control; assert the command applied and one named reaction, print the rest. Name its reaction class. The classes (settled 2026-10-18 by the retirement pass, 41 → 20; a new scenario replaces its class's row or names the new class):

| class | scenario |
|---|---|
| gang succession | `god_decapitate_gang` |
| a gang's ladder (2026-10-19; was a gang's civil war, `god_decapitate_close_succession`) | `god_decapitate_lieutenants` |
| a gang wiped | `god_kill_gang` |
| unlimited resources | `god_city_takeover_by_force` |
| chrome and violence | `god_chrome_everyone_2` |
| the Virt plane and the tech tree | `god_wipe_zetatech_day_20` |
| grudges, Hunts, vendettas | `god_kill_friend_of_leaders_day_10` |
| the outside world's demand | `god_export_10x` |
| corp succession | `god_kill_exec` |
| the corps' money | `god_wipe_corps` |
| a district riot | `god_riot_sump_west` |
| housing | `god_evict_sump_rent_10` |
| the hit market | `god_kill_ten_fathers_day_10` |
| famine | `god_famine_synth` |
| transit | `god_line_cut` |
| the School (2026-10-19; was the board's levers, `god_call_vote_habitat_after_coerce`) | `god_kill_all_teachers` |
| the Camp (2026-10-19; was captives, `god_hostage_captain_spouse_release`) | `god_child_camp_sweep` |
| the State | `god_murder_wave` |
| the law's reach, 20k (2026-10-19; was power and the utilities, `god_power_cut`) | `god_cut_the_precinct` (`god_20k.rs`, milestone tier) |
| the ages (2026-10-19; was weapons on the street, `god_flood_guns`) | `god_retire_everyone_over_55` |

Controls: `god_control`, `god_corps_control`, `god_districts_control` (`god_cut_the_precinct` carries its own no-station city). Write-ups: `docs/GOD_SCENARIOS_V12.md` (live; its "Retired 2026-10-18" section lists the 21 cut with the unit test that carries each, "Retired 2026-10-19" the five the Lives at work gate swapped), V1-V11 in `docs/history/`.

**The behaviour tier** (`citysim-cli/src/shadow.rs`, `CHECKS`): each row's bound sits a cushion outside the range the row read on main and seven config butterflies (`[lod] stat_violence_mult` 0.98-1.03, `max_coarse` 149/151, via `CITYSIM_ASSETS`). Cushions: walking + max(1 h, 15 %), the longest nightly block − 0.5 h, energy 0 + 2 h a week, known contacts met x 0.6, commute aborts + 0.05 a day, paid of worked and waged workdays − 0.1, the reporter's work hours x 0.5; refunds ≤ 1 a week and the fighter in the pit ≥ 3 of 4 bouts are absolute. **When it fails**: read the row's per-seed lines, then the diaries (`--out`). Re-calibrate only when the city changed for a wanted reason: eight runs (main and the seven butterflies, ~1.5 min each), each row's lowest and highest pooled value plus the cushions, old and new ranges in the commit message. One row that moved for a known reason is widened from one run. A stat-table regeneration should pass without one; that is the point.

## Profiling and the big map

`--features prof` (`citysim/src/prof.rs`): `run` prints a `PROF` line per timed site; `--prof-from DAY` zeroes it at that day. A scaled city: `run --population N --scaled [--rph R] [--map FILE]`. The synthetic island: `cargo test --release -p citysim --test big_island -- --ignored --nocapture` (`CITYSIM_BENCH_POP`, `_DAYS`, `_FULL`). The stat table: `cargo run --release -p citysim-cli -- calibrate` (deterministic, ~2 min) when Full-tier hourly behaviour moves, then `core_sanity`, `core_year` and the behaviour tier again; a regen redraws rare rates, so check Murders against main first.
