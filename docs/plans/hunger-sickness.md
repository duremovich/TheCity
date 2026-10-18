# Plan: hunger as sickness

Spec: [docs/HUNGER_SICKNESS.md](../HUNGER_SICKNESS.md). Decisions H1-H6 live there. Plans normally live in `~/.claude/plans/`. This one is committed in the repo so it travels with its branch: copy it there if the orchestrator prefers.

**How to run it** (the plan-implementation SOP): one Opus coder implements the plan solo in a worktree off main, with **one commit per phase**. It runs the trio per phase and the merge set at P2, P4 and P5. A single `/code-review high` over the whole range comes at the end, then a fix round. The work is tightly coupled: one `Health` component, one diet function and shared LOD hooks. A multi-agent team would re-read the same files five times.

**Toolchain** (HANDOFF): bash `export PATH="$HOME/.cargo/bin:$PATH"`; LF files; `git add -A -- . ':!.claude'`; start with `git merge --ff-only main` and confirm the SHA. The trio is:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
```

**The merge set** (run one at a time, on a quiet box):

```
cargo test --release -p citysim --test core -- --ignored core_year --nocapture
cargo test --release -p citysim-cli -- --ignored behaviour --nocapture
cargo test --release -p citysim --test god --test god_corps --test god_districts -- --ignored --nocapture
cargo test --release -p citysim --test lod -- --ignored --nocapture        # parity (P3, P5)
cargo test --release -p citysim --test core -- --ignored core_throughput --nocapture   # floor 4,000, alone
```

**The flipped config** used in the acceptance runs is a scratch copy of `assets/` (never edit the shipped `assets/config.toml` before the flip):

```
F="$SCRATCH/flip_assets"; rm -rf "$F"; cp -r assets "$F"
sed -i 's/^wages = false/wages = true/; s/^no_safety_net = false/no_safety_net = true/' "$F/config.toml"
sed -i '/^\[sickness\]/,/^\[/ s/^enabled = false/enabled = true/' "$F/config.toml"
CITYSIM_ASSETS="$F" cargo run --release -p citysim-cli -- calibrate --out "$F/stat_table.toml"
CITYSIM_ASSETS="$F" cargo test --release -p citysim --test core -- --nocapture core_sanity
CITYSIM_ASSETS="$F" cargo test --release -p citysim --test core -- --ignored core_year --nocapture
CITYSIM_ASSETS="$F" cargo run --release -p citysim-cli -- shadow --assert --out "$SCRATCH/shadow_flip"
for s in 42 43 44; do CITYSIM_ASSETS="$F" cargo run --release -p citysim-cli -- run --days 120 --seed $s --report --events > "$SCRATCH/f$s.csv" 2> "$SCRATCH/f$s.tsv"; python tools/analyze_run.py "$SCRATCH/f$s.csv" "$SCRATCH/f$s.tsv"; done
```

The "sickness only" config (the dole city with `[sickness]` on) is the same copy without the first `sed`. Every phase that touches the switches also proves **identity**: with `[sickness] enabled = false` (and, for P1, `hunger_fixes = false`), seed 42 reaches the same world hash on day 30 as main. This is a new test, `tests/sickness.rs::test_off_is_identity`, which compares against a pinned hash recomputed from main at phase start. The save must also be byte-identical.

---

## P1: the Full-planner gaps (`[life] hunger_fixes`, on)

The gaps get fixed first, before anything is calibrated against them. They are behind `[life] hunger_fixes`, which is on by default (`violence_fixes` pattern), with `hf_food_first`, `hf_mission`, `hf_buy_not_steal`, `hf_beg_retry` and `CITYSIM_HFIX_OFF=1..4`.

**Probes first.** Each probe is written and run on main, and must **fail** before its fix lands. If one passes on main, the diary's gap has another cause: trace it with the shadow diaries (`shadow --pick homeless,worker --out`) and rewrite the probe before fixing.

- `person.rs::probe_starving_buys_food_before_the_hotel` (G1): 20:00, hunger 0.05, coins = meal + bed, a Hotel nearer than the Market. Food is bought before any CheckIn; no Enjoy/Gamble before the meal.
- `person.rs::probe_broke_lawful_eats_at_the_mission` (G2): lawfulness 0.8, 0 coins, empty pantry, hunger 0.3, a stocked Mission at 25 tiles. An `EatAlms` within 6 h.
- `person.rs::probe_starving_with_coins_buys_not_steals` (G3): lawfulness 0.1, hunger 0.05, coins 3 × price, a stocked Home nearer than the Market. No `Theft` by the agent in 12 h; one `BuyFood`.
- `person.rs::probe_failed_beg_is_retried` (G4): broke, at a Market with no passer who has more than 10 coins. Another Beg (or a Scavenge) begins within 2 h, with no Earn cooldown after one dry beg.

**Files**:
- `config.rs` / `assets/config.toml`: the `[life]` keys (spec § 8).
- `systems/fixes.rs`: `hf_on(world, item)` with the env parse.
- `utility/goals.rs`: G1, the eat-first gate on Unwind/Shop/GetHigh/Socialise(Donate)/Sleep-with-CheckIn; G2, Eat's can-obtain gate.
- `systems/street.rs` / `systems/life.rs`: G1, CheckIn's reserve.
- `goap/actions.rs`: G3, `steal_mods`, with `PlanCtx.coins`/`price` already present.
- `exec/actions.rs::beg`: G4, the dry hour; `Brain.beg_dry` with serde default.
- `charity.rs::alms_plan`: G2, `hungry_reach_mult`.

**Unit tests that fail on old code**: the four probes, plus `fixes.rs` tests that `CITYSIM_HFIX_OFF=n` restores old behaviour for each item (an identity run with all four off).

**Acceptance**: the trio, plus the merge set. Behaviour rows that move are re-calibrated by TESTING.md's eight-run rule. The commit records the before/after ranges, and the moves should be the homeless and worker `known/wk` and `walk_h/d` rows only. Also check `core_sanity` default starvation and thefts against main (seeds 42-44, printed). **Do not regenerate `assets/stat_table.toml` here**: that is P5's single calibrate.

## P2: diet and the meters (`[sickness] enabled`, `diet`; off by default)

**Files**:
- New `systems/diet.rs`: `quality`, `meal_price`, `synth_price`.
- New `systems/health.rs`: `Health`, `MealTally`, `ate`, `hourly_mark`, `daily`, `effects`.
- `components.rs`: `Health`, `Inventory.synth`, `Building.synth_food`, `Market.synth_frac`, `Charity.synth_frac`.
- `config.rs`: `SicknessCfg` with `off()`, and `v1_profile` pins it off.
- `systems/economy.rs`: `buy_quantity`, `pay_for_food` and `take_food` take a `Quality`; the shelf accumulator; spoilage skips synth.
- `exec/actions.rs`: the eat sites call `health::ate`; real is eaten first.
- `systems/lod.rs`: `stat_eat` buys by `diet::quality`; the hourly mark.
- `needs.rs`: the hourly mark on bodies, `starve_grace_days`, `DecayCtx.energy_mult` composed.
- `systems/charity.rs`: synth meals, the accumulator, the `Alms` cost.
- `systems/life.rs` / `systems/leisure.rs`: `meal_price` reads `diet::meal_price`.
- `mood.rs` (bias); `demography.rs` (fertility, old age); `systems/law.rs` (`resolve_fight` frailty).
- `systems/economy.rs` + `systems/jobs.rs`: work output; `exec/routine.rs` + `lod::stat_work`: the sick day.
- `stats.rs`: `HealthCols`, appended before `ticks_per_sec`.
- `save.rs`: `SAVE_VERSION = 7`.
- `citysim-cli/src/shadow.rs`: the Health header line, meal lines, the `sick` pick.
- `tools/analyze_run.py`: the "health" section.

**Unit tests that fail on old code** (new `tests/sickness.rs`):
- `test_off_is_identity`: the hash and the save bytes.
- `test_synth_meal_costs_a_third_and_drains_a_third` (price 4: 1 coin, 3 meals per unit).
- `test_real_bought_above_the_floor`.
- `test_mission_serves_three_synth_meals_per_unit`.
- `test_meters_daily_from_the_tally` (all-synth day, hungry day, real day; exact arithmetic).
- `test_effects_reach_energy_work_mood_fertility`.
- `test_sick_day_skips_the_shift_on_both_tiers` (a Full worker and a Statistical one with the same `sick_today`: neither paid).
- `test_statistical_tally_matches_full` (one agent, one day each tier, same meals: same meters).
- `test_starvation_takes_seven_days_with_sickness`.
- `test_save_round_trip_with_health` (in `save.rs`).

**Acceptance**:
- The trio (default city: identity holds).
- The "sickness only" config, `core_sanity` and one 120-day run per seed: print synth share, mean malnutrition/toxicity, `malnourished`, starvation.
- `core_throughput` alone: the floor is 4,000 ticks/s, and the cost of the hourly mark plus the daily pass is printed against main.

## P3: the bins (`[sickness] bins`)

**Files**:
- New `systems/bins.rs`: `fill_daily`, `rot`, `nearest_stocked`, `contest`, `dive`, `stat_dive`; the `World::bins` index rebuilt in `rebuild_indices`.
- `components.rs`: `Building.bin`, `bin_regular`.
- `goap/actions.rs`: `ActionKind::DumpsterDive` appended to the enum and to `PLANNABLE` (87 → 88), with preconditions, cost and effects (`hunger_satisfied`); `LocationKey::Bin` in `world_state.rs` plus the `PlanCtx.bin_stock`/dist binding.
- `exec/actions.rs`: duration 20, `can_start`, `on_complete`, the reservation.
- `utility/goals.rs`: Eat's gate opens on a bin in reach.
- `systems/lod.rs::stat_eat`: `p_dive`, before `p_desperate`.
- `world.rs::StatTable.p_dive`.
- `citysim-cli/src/main.rs` (calibrate): count Full dive starts per eligible agent-hour per row, written only with `[sickness] bins` on.
- `events.rs`: `Dove`, `BinContest`.
- `systems/litter.rs` (the dive's deposit); `systems/moves.rs` callers (Intimidate); the extortion/assault path for the gang shakedown.
- `tests/planner_key.rs` / `tests/goap.rs`: update the plannable count.
- `lod.rs`: the parity test gains the diet rates.

**Unit tests that fail on old code**:
- `test_market_bin_fills_from_spoilage_and_rots`.
- `test_home_bin_from_pantry_spoilage`.
- `person.rs::probe_broke_with_no_mission_dives` (0 coins, no Mission in reach, a stocked bin at 20 tiles: a dive meal within 6 h).
- `test_regular_wins_or_loses_by_intimidate` (both outcomes over seeds).
- `test_gang_turf_shakes_down_a_non_member` (toll paid; no coins → Assault; once a day).
- `test_member_dives_free`.
- `test_stat_dive_uses_the_same_contest`.
- `test_calibrate_writes_p_dive_only_with_bins`.
- `test_one_diver_per_bin`.

**Acceptance**:
- The trio.
- The "sickness only" `core_sanity` with new mechanism bullets checked to fire on 4-6 of 42-47 before they are added ("a synth meal", "a dive", "a malnourished agent" as a `day_mark`, "a BinContest"). One that fires on 2 or fewer stays a unit test (TESTING.md).
- The parity test on the "sickness only" config after a scratch `calibrate`: the diet rates within 15 % (else fix the shared function, never the bound).
- One shadow run on that config with diaries (`--pick homeless,worker,gang_member,sick --out`), read by the coder. In the commit: one paragraph per archetype on what they eat and where.

## P4: cancer and the Clinic (`[sickness] cancer`)

**Files**:
- `systems/health.rs`: the onset roll, the course, diagnosis, the death.
- `components.rs`: `Cancer`, `DeathCause::Disease` (appended), `MemoryKind::Diagnosed`, `Dove`.
- `goap/actions.rs`: `ActionKind::Oncology`, scripted under Treat like Therapy/Detox (not in `PLANNABLE` if Treat's steps are scripted; else appended).
- `utility/goals.rs`: Treat's considerations and `already_satisfied`; Earn's savings line raised to the price while diagnosed.
- `systems/chrome.rs` (or a new `systems/oncology.rs`): the price, `Flow::Treatment` to the owner, cure/remission.
- `systems/lod.rs` or `health.rs`: `p_stat_treat`, the daily off-screen treatment.
- `events.rs`: `Diagnosed`, `Cured`.
- `stats.rs`: the cancer columns, `deaths_disease`.
- `systems/news.rs` / `gossip`: a Disease death is a deed like any death (check it reaches stories; add nothing if it does).
- `shadow.rs`: the diagnosis line.

**Unit tests that fail on old code**:
- `test_cancer_rolls_only_above_tox_min` (a seeded world: 1,000 agent-days at t 1.0 gives a count near `cancer_p`).
- `test_stage_grows_and_kills_at_one_as_disease`.
- `test_diagnosis_at_symptom_stage_writes_event_and_memory`.
- `test_treatment_cures_below_cure_stage_and_remits_above`.
- `test_treatment_coins_reach_the_clinic_owner` (coin identity).
- `person.rs::probe_diagnosed_patient_with_coins_seeks_the_clinic` (stage 0.3, coins 300: an `Oncology` within 2 days).
- `test_stat_patient_treats_on_p_stat_treat`.
- `test_saving_for_treatment_raises_earn`.

**Acceptance**: the trio; the merge set on the default config (identity: nothing moves); `core_year` on the "sickness only" config (printed: Disease deaths, cancers, cures, starvation per 30-day window).

## P5: calibrate, measure the flip, docs

The single `calibrate` and the judge-by numbers. **This phase does not flip the shipped config.** The flip is P9a's (pre-approved if this phase's numbers pass), as its own commit after this plan merges.

1. **The table.** Rebase or redo the `recal` tree's calibration city (the shipped city at 2,000: corps, rent, tiered wallets) on top of P1-P4. Regenerate `assets/stat_table.toml` for the default config: P1's fixes move the Full tier. Generate the flipped table in `$F` as above. In the commit: thefts, starvation and Murders against the all-Full ground truth (`--lod all-full` if the recal tree's flag exists, else the parity test's Full side).
2. **J24/J25**: port them from P9a's tree (`agent-a82410b104eb008ad`, `tests/core.rs`) into `core_sanity`/`core_year`, read only under `econ::no_net`, so the default city is unaffected. From day 30: employed ≥ 50 % of free adults, and homeless ≤ 25 % of adults.
3. **Behaviour tier**:
   - Add `starve_h/wk`, `synth%` and `dive/wk` to `shadow.rs`'s metrics (printed first).
   - Re-calibrate the bounds by TESTING.md's eight-run rule on the default config.
   - Bound `starve_h/wk` for the homeless, worker and gang_member archetypes, as a cushion above the ensemble's max.
4. **One god scenario**: `god_famine_synth` in `god.rs`. On day 45 the Warehouse and every Market shelf are emptied, against its control. Asserted: starvation stays under the control's + 5, and dive meals rise. Write-up in `docs/history/GOD_SCENARIOS_V9.md`.
5. **Docs**: the spec's "Implemented: deviations"; TESTING.md (the new bullets, metrics and god scenario); HANDOFF session log; memory `thecity-hunger-as-sickness.md` (state).

**The judge-by runs** (all on the flipped config `$F`, plus the same on the "sickness only" config for the dole city):

| what | command | reads (printed, the commit records them) |
|---|---|---|
| core | `CITYSIM_ASSETS="$F" cargo test --release -p citysim --test core -- --nocapture core_sanity` | green with J24/J25; starvation per seed (target: near zero, single digits); Disease deaths; synth share |
| year | `CITYSIM_ASSETS="$F" cargo test --release -p citysim --test core -- --ignored core_year --nocapture` | every 30-day window holds; population d365; starvation and Disease deaths per window |
| behaviour | `CITYSIM_ASSETS="$F" cargo run --release -p citysim-cli -- shadow --assert --out "$SCRATCH/shadow_flip"` | rows pass; the homeless/worker/sick diaries show synth meals, dives, bin contests and a diagnosis or a sick day (quote one of each in the commit) |
| probes | `CITYSIM_ASSETS="$F" cargo test --release -p citysim --test person -- --include-ignored` | the Jobs probes P9a un-ignores, plus this plan's probes |
| parity | `CITYSIM_ASSETS="$F" cargo test --release -p citysim --test lod -- --ignored --nocapture` | the diet rates within 15 % |
| runs | the 42-44 × 120 `run --report` loop above + `analyze_run.py` | the health section; J24/J25 per day; starvation vs flip readiness's 145-205 |
| floor | `cargo test --release -p citysim --test core -- --ignored core_throughput --nocapture` (default and `$F`, alone) | ≥ 4,000 ticks/s on the default config |

**Stop rule**: if the flipped core_year fails a window, or starvation on 42-44 stays above ~20 in 120 days, do not tune `[sickness]` to pass. Trace who died, and from which gap, with the diaries (G1-G4 or a missing bin in reach), fix the mechanism, and report to the orchestrator. Calibration of the sickness keys (cancer volume, toxicity rates) is a printed finding for the late calibration milestone.

Then: `/code-review high` over P1..P5, an Opus fix round, the merge set on main, the handoff, and the flip (P9a re-run, then P10 retiring the switches).

## Risks

- **The planner key and A\***: a new plannable action and `LocationKey` widen the search (M15 lesson). Check the A\* cap counter in the `run --report` budget columns before and after P3.
- **Throughput**: hourly marks are on every agent, and the bin lookup runs per Eat think. Use the index, never scan (`no_per_tick_scan.rs` must cover `bins`).
- **Food supply**: synth stretch triples effective food for the poor, so the Winter food-theft wave will shrink. That is a city change, not a bug. Do not restore it by knob (memory `thecity-brutality-via-pressure.md` is Dylan's call, open question 2).
- **Parity**: `p_dive` learned in a calibration city with few hungry agents can be noisy per row. If a row has under ~200 eligible agent-hours, fall back to the lawfulness-bucket mean (record it in the table as the row's value; no new field).
- **The behaviour tier flips on city change** (any phase moves picks): the reworked tier pools 30 agents a seed, but P1 and P2 both move hunger. Budget two re-calibrations (P1, P5).
- **Coin identity**: tolls, treatment and synth payments all go through `ownership::pay`. A new `Flow` gets a ledger line (`Flow::Toll` if the shakedown is not an Extort).
