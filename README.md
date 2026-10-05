# TheCity

A living city simulator: one city, 2D top-down squares, utility-based goal selection over a GOAP planner, with economy, law, social graph and demography as world systems.

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

## Milestones

- **M0-M7 (v1):** the single-gang city: map, agents, GOAP planner, economy, law, social graph, demography, the debug UI. See [docs/SPEC.md](docs/SPEC.md).
- **M8 factions:** two gangs with a brain that issues orders (expand, contest, raid, retaliate, lie low), turf, raids and sacks. See [docs/M8_FACTIONS.md](docs/M8_FACTIONS.md).
- **M9 the law:** the Jail's guards become a third faction with a captain and postures (patrol, crackdown, garrison), breakouts, bribery and a night watch at each Hideout. See [docs/M9_LAW.md](docs/M9_LAW.md).
- **Next:** M10 scale and M11 ownership are specced in [docs/M10_SCALE.md](docs/M10_SCALE.md) and [docs/M11_OWNERSHIP.md](docs/M11_OWNERSHIP.md); the end state is in [docs/VISION.md](docs/VISION.md).

## Verify

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release --no-fail-fast
# long scenario gates (v1 acceptance, M8, M9, Full-vs-Statistical parity), about a minute
cargo test --release -p citysim --test scenario --test lod -- --ignored --nocapture
# regenerate the Statistical tier's table (assets/stat_table.toml), about 2 s
cargo run --release -p citysim-cli -- calibrate
# debug run with the event log (events go to stderr)
cargo run --release -p citysim-cli -- run --days 120 --seed 42 --report --events > run.csv 2> events.tsv
```
