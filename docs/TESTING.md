# Testing

2026-10-09 (Dylan): "The most important thing is emergent story. For that we need people that appear to act like people." The suite was rebuilt around that: a fast core that catches a *broken* city, a behaviour tier that judges whether people *act like people*, and a short list of god scenarios. Everything else that used to be asserted (calibration bands, multi-seed trajectories, numbers pinned to an older city) is gone.

**The doctrine, in three lines:**

1. Asserts catch broken: a panic, nondeterminism, a coin made or lost, a collapse, a mechanism that never fires.
2. Behaviour judges alive: the archetypes' diaries and the person probes are the primary judge.
3. Numbers are reports, not gates: `tools/analyze_run.py` prints them from `citysim-cli run --report`.

## The tiers

| tier | what | where | asserts | wall time (this box, release, alone) |
|---|---|---|---|---|
| unit | every hand-built test (one mechanism, a small or seeded world) | `citysim/tests/*.rs`, `citysim-cli` | the mechanism does what it says | ~40 s summed over the binaries (all non-ignored but `core_sanity`) |
| core | `core_sanity` (non-ignored) | `citysim/tests/core.rs` | seeds 42-44 x 120 days in threads, one collector: coin identity every day, collapse bounds, 33 mechanism-existence bullets, the ticks/s floor on seed 42 | ~30 s |
| core | `core_year` (`#[ignore]`) | `citysim/tests/core.rs` | seed 42 x 365 days, the collapse bounds per 30-day window | ~100 s |
| core | determinism and saves | `determinism.rs`, `save.rs` | same seed same hash at a day boundary; a mid-day save runs on byte for byte (plus the hunt, guard and contract rebuilds) | seconds |
| behaviour | `shadow --assert` / `test_behaviour_tier` (`#[ignore]`) | `citysim-cli/src/shadow.rs` | 146 per-archetype bounds on the diary metrics (seed 42, days 19 and 90, 5 picks each) | ~25 s |
| behaviour | person probes | `citysim/tests/person.rs` | one pinned agent, one stimulus, what a person would do | ~2 s |
| god | 13 scenarios + 3 controls (`#[ignore]`) | `god.rs`, `god_corps.rs`, `god_districts.rs` | the world reacted to the shock (or the command applied), 60 days | ~55 s |
| nightly | Full-vs-Statistical parity (`#[ignore]`) | `lod.rs::test_full_vs_statistical_within_15pct` | the Statistical tier's rates within 15 % of Full | ~25 s |

Measured 2026-10-09 (after the off switches retired): `cargo test --workspace --release` (every non-ignored test, `core_sanity` included) ran in **1 min 17 s** after compile (653 passed, 19 ignored).

## When to run what

- **Every change** (the per-change loop, ~1.5 min after compile):
  ```
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --release
  ```
- **Every merge** (the loop plus the year, behaviour and god, ~4 min more; run them one at a time, each spawns its own threads):
  ```
  cargo test --release -p citysim --test core -- --ignored core_year --nocapture
  cargo test --release -p citysim-cli -- --ignored behaviour --nocapture   # or: citysim-cli shadow --assert
  cargo test --release -p citysim --test god --test god_corps --test god_districts -- --ignored --nocapture
  ```
- **Every milestone** (or nightly): the merge set, the parity test, and a human (or model critic) read of the diaries:
  ```
  cargo test --release -p citysim --test lod -- --ignored --nocapture
  citysim-cli shadow --assert --out shadow_out     # keeps the diaries under shadow_out/d19, shadow_out/d90
  ```
  plus a shadow pass in the style of `docs/SHADOW_V2.md` (critics read one diary per archetype and write what would make it real). That read is the final judge of "alive"; the bounds only catch regressions between reads.

## The core tier

`core_sanity` runs the shipped config (`Config::load()`) on seeds 42, 43 and 44 for 120 days, each in its own thread, and walks every event by id cursor once an hour into one collector per seed. It fails on:

- **the coin identity** (`econ::identity`: `total_coins + Σ outside treasuries − minted`) moving on any day;
- **collapse**: population outside 1,333..=2,667 on any day; starvation over 200 in 120 days; Assault + Murder events over 42.7 a day; the Treasury below 0 from day 30; the Jail over capacity; every Market empty three days running; fewer than 4 seeded corps alive at the end; any prisoner held at the Statistical tier starving;
- **a mechanism that never fired** on any of the three seeds (the list below);
- **ticks/s** under 4,000 on seed 42 (release only). The core binary holds one non-ignored test and cargo runs test binaries one at a time, so the seed runs alone but for its two sibling threads (it reads ~6,000 there; ~10,700 truly alone and idle).

The bounds are the v1 sanity trio scaled to 2,000 residents (M10 plan D38), unchanged since M8. `core_year` runs the same bounds over each 30-day window of a 365-day seed-42 run, the Assault bound at 1.5 x (a 30-day window is noisier than 120 days; seed 42 peaks at 36.3).

The mechanism bullets (33, one per milestone's core mechanism, measured on 42-47; each fired on 4-6 of the 6 seeds): a Marriage, a GangJoin, a Birth, a Burial (M5, M6); a raid resolved, a Home flipped (M8); a Jailbreak, a day in Crackdown (M9); a hole bound (M10); an eviction, an NPC founding, a hostile takeover, a Strike (M11); a Riot, a gang Split, a Squat (M12); a Crash, a chrome Install, a Dealing report (M13); a run, a Data sale, a Flatline (M14); a Feed Story, a Hunt, a Vendetta, a Purist expulsion (M15); a World export, a Collect, a Bout, a gang front, a Mission meal (L2, the Real economy); a BountyPaid, a city guard on the take (M16a phase 3).

### Adding a mechanism-existence bullet

1. Run `core_sanity --nocapture`: it prints every event kind's count and every mark per seed.
2. If the mechanism is an event kind, add `("a Thing", k(EventKind::Thing))` to `mechanisms()` in `core.rs` with the 42-44 counts in a comment. If it needs more than a kind (an event's text, a day-end state, a `DayRow` column), add a mark in `run_seed` (`Some("my mark")` in the event match, or `day_mark(...)` at the day's end) and `m("my mark")`.
3. **Rare mechanisms get a unit test instead.** If it fires on 2 or fewer of 6 seeds (42-47) in 120 days, do not add a bullet (it will flip with every unrelated change). Write a unit test in a seeded world that sets up the preconditions and calls the system directly. Examples: `tech.rs::test_research_held_builds_a_lab_in_the_focus` (a Lab under Research, 2 of 6 seeds), `camp.rs::test_unfed_child_taken_at_take_days_before_death` (a child taken, 0 of 6), the contract posting tests in `contracts.rs` (1 of 6).

Never add a band. If a number matters, print it in `run --report` and read it with `tools/analyze_run.py`.

## The behaviour tier

### `shadow --assert`

`citysim-cli shadow --assert [--out DIR]` (or `test_behaviour_tier`) runs the V2 shadow pass's pinned windows: seed 42, 7 days from day 19 (a Friday) with 5 of each of 17 archetypes, and 7 days from day 90 with 5 of each gang archetype, the two windows in parallel. Each pinned agent's diary metrics are summed per archetype:

| metric | what |
|---|---|
| `walk_h/d` | travel-class hours a day |
| `sleep_h/d` | sleep-class hours a day (printed) |
| `longest_h` | the mean over agent-nights (noon to noon) of the night's longest sleep block |
| `e0_h/wk` | hours at energy 0 per agent-week |
| `work_h/d` | work-class hours a day |
| `worked` | whole workday shifts (free) with half the shift at work-class steps (printed) |
| `paid/wkd` | worked shifts paid a wage by two hours after the end |
| `waged_wd` | whole workday shifts (free) with a wage |
| `hang/wk` | HangOut steps per agent-week (printed) |
| `known/wk` | distinct known contacts met at a HangOut per agent-week |
| `refund/wk` | purchases refunded (a started buy undone) per agent-week |
| `abort/d` | Work goals dropped before a single work-class minute (a commute turned back) per agent-day |
| `bouts_at` | bouts with the fighter inside the pit, of the bouts naming her |

The bounds (`CHECKS` in `shadow.rs`, each with the value measured beside it, last on the city with the off switches retired and the stat table regenerated) are set so today's city passes with a margin: walking ≤ 1.4 x + 1 h, the longest nightly block ≥ 0.7 x, energy 0 ≤ measured + 6 h a week, known contacts ≥ 0.5 x (where ≥ 2), refunds ≤ 1 a week, commute aborts ≤ 1.5 x + 0.5 a day, paid of worked ≥ measured − 0.25, waged workdays ≥ measured − 0.3, the reporter at her desk ≥ 1 h a day, the fighter in the pit for ≥ 3 bouts in 4. They catch a regression in "acts like a person" (V2 causes 4, 6, 7, 10, 18, 20 come back); they are not targets. An archetype with no candidate in a window is a SKIP, not a failure. When a behaviour fix moves a number for the better, re-measure (`shadow --assert` prints the table) and tighten the bound in the same change. Five picks per archetype (three until 2026-10-09: a regenerated stat table swapped the picks and moved several rows both ways at once) is still a small sample. Read the diaries of a failing row before calling it a regression; a re-measure after a deliberate city change is recorded with the old and new values in the commit message.

### Person probes

`citysim/tests/person.rs`: the shipped seed-42 city, one agent pinned at Full LOD, one stimulus, and an assert on what a person would do. Today's six (all pass):

- hungry, broke, an empty larder at 15:00: paid at 18:00, buys food before midnight's rent;
- married, across town at 20:00, tired: at home at 03:00;
- an Enemy standing at the nearer HangOut spot: the other spot;
- at the counter at 17:45, a little hungry: stays to 18:00 and is paid;
- an immigrant with no coins and no food: eats or is hired within five days, alive;
- a guard starting a twelve-hour shift hungry: eats on shift, never at hunger 0.

### Adding a person probe

Copy a probe: build `World::new(42, Config::load())`, run to the hour you need, pick an agent with the helpers (`day_workers_at_work`, the civilian filters), `pin` it, set the stimulus through components (`Needs`, `Wallet`, `Inventory`, `Position`, edges), run with `run_to` (it re-pins every tick and lets you watch each tick), then assert the outcome a person would pick. Read the outcome from the agent's own state (its step, its wallet, its building), never from a city-wide count. If the probe exposes a real gap, mark it `#[ignore]` with the gap in a comment and list it for the behaviour round; do not fix behaviour in the test change.

## God scenarios

The 13 kept, one per reaction class, seed 42, 60 days (the shock at day 45, or day 10/20 in the v5/v6/v7 harnesses), each against its unshocked control: `god_decapitate_gang` (succession), `god_kill_gang` (a gang wiped), `god_city_takeover_by_force` (unlimited resources), `god_bankrupt_city` (the city's money), `god_crackdown_forever` (the law pinned), `god_chrome_everyone_2` (chrome and violence), `god_wipe_zetatech_day_20` (the Virt plane and the tech tree), `god_kill_friend_of_leaders_day_10` (grudges, Hunts, vendettas), `god_export_10x` (the outside world's demand), `god_kill_exec` (corp succession), `god_wipe_corps` (the corps' money), `god_riot_sump_west` (a district riot), `god_evict_sump_rent_10` (housing). The other 40 were near-duplicates of these (the same lever class, or a lever with its own unit test). Write-ups: `docs/GOD_SCENARIOS_V1..V7.md`.

## What was retired (2026-10-09)

`tests/scenario.rs` (4,746 lines: 12 milestone gates re-running the same city ~50 times, ~50 calibration and trajectory bullets, the pinned murders/starvation/FNV constants, the majority and six-seed devices, eight copies of the sanity trio and the floor, the M1/M0/M5/M6 short scenarios), `tests/scratch_probe.rs` (33 ignored print-only probes), the 13 legacy-save loaders and the v1-profile bit-identical save in `save.rs`, four of the five `lod.rs` ignored tests (kitted parity, leisure parity, faction-violence parity, the stat-policy bench) and its non-ignored throughput copy, `scale::test_tick_2000_median_under_250us`, `outside::probe_coin_census`, 40 god scenarios.

## The switches retired (2026-10-09)

Every feature is always on except the work in flight: `[economy2] wages` and `no_safety_net` (off by default), and `[life] violence_fixes` with its `vf_*` items and `CITYSIM_VFIX_OFF`. Gone: `--virt-off`, `--word-off`, `--life-off`, `--l2-off`, `--contracts-off`, `--econ-off` (and `shadow --life-off`), `Config::{virt_off, word_off, living_off, econ_off, contracts_off, with_leisure}`, the `enabled` master of `[life]`, `[living]`, `[jobs]`, `[leisure]`, `[budget]`, `[fviolence]`, `[gossip]`, `[hunt]`, `[moves]`, `[competence]`, `[news]`, `[virt]`, `[contracts]`, `[economy2]` (and its `market`), `[world_market]`, `[charity]`, `[camp]`, `[treasury]` and `[export]` (with L2's flat-price export hook the market replaced), `[lod] budget`, `[gossip] legacy_second_hand`, `[life] l2_fixes` with its six sub-switches and `CITYSIM_L2FIX_OFF`, every off branch behind them, their identity tests, and the legacy-save migrations (`World::migrate_legacy`, the per-system `migrate` hooks, the `RivalHideout` alias): a save from before format 4 no longer loads. `v1_profile` (the 300-resident unit-test city) and `calibration_city` now run the life pass, the plane, the word, the living city, contracts and the World market; they keep only their own cuts (the v1 map and economy; gangless, rent-free, no riots or assets), pin the in-flight `[economy2] wages` and `no_safety_net` off (an experiment in `config.toml` never reaches the unit-test city or the stat table), and follow the shipped default for `[life] violence_fixes`. `calibrate` builds `assets/stat_table.toml` in that city: regenerate it (`cargo run --release -p citysim-cli -- calibrate`, deterministic, ~15 s) when the Full tier's hourly behaviour moves, then re-run `core_sanity`, `core_year` and the behaviour tier on it.
