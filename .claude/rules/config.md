---
paths:
  - "assets/config.toml"
  - "assets/stat_table.toml"
  - "citysim/src/config.rs"
---

# Config keys

- **Every number a system reads is a key in `assets/config.toml`**, in the section of the system that reads it, with a trailing comment like its neighbours (what it is, the unit, why this value or where it was measured). The `Config` struct in `citysim/src/config.rs` mirrors the file with `serde(default)` so an old config still loads.
- **A new section names the system that reads it** in its header comment, and `docs/ARCHITECTURE.md`'s config table gets a row.
- **Names are data**: corp, gang, district and creed names live in config rows; code looks them up by role or index, never by string (`Config::unresolved_names` must return empty; `names.rs` asserts it).
- **Never change an earlier milestone's value to pass a gate**; a value change is its own commit with the before/after numbers on seeds 42-44.
- **Profiles are methods on `Config`**: `dole_city()` (the old default, used by most unit tests), `v1_profile()` (the 300-resident test city), `calibration_city(n)`, `scaled_to(n)`. A test pins the city it was written for; new behaviour tests run the shipped default.
- **`assets/stat_table.toml` is generated** (`cargo run --release -p citysim-cli -- calibrate`, ~2 min, deterministic). Regenerate it when Full-tier hourly behaviour moves, then re-run `core_sanity`, `core_year` and the behaviour tier; a regen redraws rare rates (Murders can move ±40 %), so check Murders against main before shipping one.
- A config file that fails to parse is a hard error at startup, not a default.
