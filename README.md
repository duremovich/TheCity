# TheCity

A living city simulator: one city of 2,000 residents on a 256 x 192 map, 2D top-down squares, utility-based goal selection over a GOAP planner, with economy, law, social graph and demography as world systems. Agents off screen live on cheaper tiers (Full, Coarse, Statistical) but still commit and suffer crimes; every agent has a biography.

The full build spec lives in [docs/SPEC.md](docs/SPEC.md).

## Layout (planned)

- `citysim` — headless simulation library
- `citysim-app` — macroquad renderer and debug UI
- `citysim-cli` — headless runner and calibration

## Running

```
cargo run -p citysim-cli -- run --days 30 --seed 42 --report
cargo run -p citysim-app
```

`citysim-cli run` flags: `--days N --seed S` (required), `--report` (daily CSV on stdout), `--events` (events on stderr), `--population N` (override `[world] population`), `--map FILE` (override the map), `--lever "day=D:name=value"` (repeatable, e.g. `day=30:law_posture=crackdown`, `law_posture=auto` releases the pin), `--save-at T`, `--load FILE`, `--force-lod full|coarse|stat`.

`citysim-app` flags: `--seed`, `--load`, `--map FILE`, `--fps`, and the screenshot helpers `--start-tick T`, `--screenshot out.png` (frames the whole map unless something is selected), `--fit`, `--select INDEX`, `--select-kind Hideout`, `--select-name "First Last"`, `--tab story`, `--no-autobind`. The City panel shows tier counts, open holes and the sim rate; the event log has an "Unattributed" toggle that lists open holes and binds one on click.

## Milestones

- **M0-M7 (v1):** the single-gang city: map, agents, GOAP planner, economy, law, social graph, demography, the debug UI. See [docs/SPEC.md](docs/SPEC.md).
- **M8 factions:** two gangs with a brain that issues orders (expand, contest, raid, retaliate, lie low), turf, raids and sacks. See [docs/M8_FACTIONS.md](docs/M8_FACTIONS.md).
- **M9 the law:** the Jail's guards become a third faction with a captain and postures (patrol, crackdown, garrison), breakouts, bribery and a night watch at each Hideout. See [docs/M9_LAW.md](docs/M9_LAW.md).
- **M10 scale:** 2,000 residents on the 256 x 192 map v2 (five zones), the Statistical tier spread over the hour, off-screen crimes recorded as holes and bound to actors by a deterministic binder, a biography (Life) for every agent. See [docs/M10_SCALE.md](docs/M10_SCALE.md) and its "Implemented: deviations" section.
- **Next:** M11 ownership is specced in [docs/M11_OWNERSHIP.md](docs/M11_OWNERSHIP.md); the end state is in [docs/VISION.md](docs/VISION.md).

## Verify

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release --no-fail-fast
# long scenario gates (v1 acceptance, M8, M9, M10 scale, Full-vs-Statistical parity, binder, median tick);
# each scenario is 25-60 s at 2,000 residents, so run them one at a time if you want readable output
cargo test --release -p citysim --test scenario --test lod --test scale --test bind -- --ignored --nocapture
# the M10 scale gate alone: throughput (release only, gate 8,000 ticks/s, target 12,000), off-screen violence,
# hole ledger, Unknown share, save size and time
cargo test --release -p citysim --test scenario test_m10_scale_seed_42 -- --ignored --nocapture
# criterion bench (tick_300_agents, tick_2000_agents); the median gate is a test in tests/scale.rs
cargo bench -p citysim --bench tick
# regenerate the Statistical tier's table (assets/stat_table.toml): calibrate v2, 500 agents, gangless, 3 seeds
cargo run --release -p citysim-cli -- calibrate
# debug run with the event log (events go to stderr)
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
```

## Tools

Stdlib-only Python scripts in `tools/` (see [tools/README.md](tools/README.md)):

```
python tools/analyze_run.py run.csv events.tsv   # one run: economy, law, holes, throughput, anomaly flags
python tools/compare_runs.py a.csv b.csv         # per-column means of two runs
```

`assets/gen_map.py` regenerates `assets/map.txt` (write bytes, LF line endings).
