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
