# TheCity

A cyberpunk city that runs by itself: 2,000 residents on a 256 x 192 map, living under owners. The end state is a place a player can go break, take over, or simply exist in, with a lot of little stories along the way. Corps own the Vat Farms, Street Markets and Blocks, gangs hold the Sump, the Precinct's guards answer to whoever bribes the captain, and districts decay as the money moves. Agents are utility-driven over a GOAP planner; off screen they live on cheaper tiers but still commit crimes and have a biography you can read. The sim is the product; the player comes after it stands on its own. See [docs/VISION.md](docs/VISION.md).

The full v1 build spec lives in [docs/SPEC.md](docs/SPEC.md).

## Layout

- `citysim` — headless simulation library
- `citysim-app` — macroquad renderer and debug UI
- `citysim-cli` — headless runner and calibration

## Running

```
cargo run -p citysim-cli -- run --days 30 --seed 42 --report
cargo run -p citysim-app
```

`citysim-cli run` flags: `--days N --seed S` (required), `--report` (daily CSV on stdout), `--events` (events on stderr), `--population N` (override `[world] population`), `--map FILE` (override the map), `--lever "day=D:name=value"` (repeatable, e.g. `day=30:law_posture=crackdown`, `law_posture=auto` releases the pin), `--save-at T`, `--load FILE`, `--force-lod full|coarse|stat`.

`citysim-app` flags: `--seed`, `--load`, `--map FILE`, `--fps`, and the screenshot helpers `--start-tick T`, `--screenshot out.png` (frames the whole map unless something is selected), `--fit`, `--select INDEX`, `--select-kind Hideout` (a Rust `BuildingKind` name: `Market` is shown as Street Market, `Jail` as Precinct), `--select-name "First Last"`, `--tab story`, `--no-autobind`. The City panel shows tier counts, open holes and the sim rate; the event log has an "Unattributed" toggle that lists open holes and binds one on click.

Display names differ from code names: Block (`Home`), Vat Farm (`Farm`), Street Market (`Market`), Precinct (`Jail`), Civic Hall (`Hall`), Recycler (`Cemetery`), Reserve Depot (`Warehouse`); money is shown as ¢. The CSV columns and the Rust identifiers keep the v1 names.

## Roadmap

| Milestone | Scope | Spec |
| --- | --- | --- |
| M0-M7 | v1: one gang, economy, law, social graph, demography | [SPEC](docs/SPEC.md) |
| M8 Factions | two gangs with a brain: orders, turf, raids, sacks | [M8](docs/M8_FACTIONS.md) |
| M9 The law | the guards as a faction: captain, postures, breakouts, bribery | [M9](docs/M9_LAW.md) |
| M10 Scale (done) | 2,000 residents, five zones, Statistical tier, holes and the binder, Life | [M10](docs/M10_SCALE.md) |
| M11 Ownership (in progress) | theme labels, owners and purses, rent and eviction, eight corps with a brain, founding, classes | [M11](docs/M11_OWNERSHIP.md) |
| M12 Districts | district aggregates, private security, litter, riots and strikes | [post-M14](docs/ROADMAP_POST_M14.md) |
| M13 Assets | vehicles, chrome, stims, the security robot | [post-M14](docs/ROADMAP_POST_M14.md) |
| M14 Data and Virt | a second plane, decks, ICE, Data as a resource, the tech tree | [post-M14](docs/ROADMAP_POST_M14.md) |
| M15 Word and blood | gossip, reputation, grudges, revenge chains | [post-M14](docs/ROADMAP_POST_M14.md) |
| M16 Contracts | hits, fixers, leverage moves, the quest board | [post-M14](docs/ROADMAP_POST_M14.md) |
| M17 The outside world | megacorp, syndicate and state parents at ledger LOD | [post-M14](docs/ROADMAP_POST_M14.md) |
| M18 The player | one more pinned agent, dialogue as a renderer over social state | [post-M14](docs/ROADMAP_POST_M14.md) |

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
# the player-lever scenarios
cargo test --release -p citysim --test god -- --ignored --nocapture
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
