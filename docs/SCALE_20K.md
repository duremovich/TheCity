# Scale: 20,000 simulated residents on the big island

Decided with Dylan 2026-10-11/12, before M16b. Source data: the scale probe (seed 42, main d14619b), summarised in
§ Baseline. Every number below marked "target" is a placeholder until measured.

| Question | Decision |
| --- | --- |
| Simulated population | **20,000** named, complete individuals (the M10 rule holds: the brain is coarse off screen, never the record). 50k is a later stretch (parallel Statistical hour). |
| The rest of "a few hundred thousand flats" | An **Ambient** tier: a flat holds a seed and an occupancy flag; districts carry aggregate demand (food, rent, labour, crime, street density). A named resident is materialised deterministically from the flat's seed on demand. |
| The map | Grows past 256 × 256. TheCityGraphics (read only, never written) is imported **through an adapter** into the sim's own map spec, because its output still evolves. The gates never read the graphics repo: they run on an in-repo map (the current text map at 2k, a generated stand-in at 20k, or a committed snapshot of an import). |
| Test floors | The 2k gate profile keeps core_sanity's 4,000 tps floor. A new 20k profile gets its own floor, set from the measured tps after S1-S3 with ~25 % headroom (this box is noisy). |
| Order | Before M16b, so later milestones are written for 20k. |

## Baseline (probe, single thread, 30 days)

| residents | tps (days 26-30) | MB | note |
| --- | --- | --- | --- |
| 2k | 4,063 | 65 | |
| 10k | 587 | 243 | |
| 20k | 171 | 429 | 27 % employed, 1,405 starved: the map lacks employers |

Super-linear: Statistical co-worker pairs (O(staff²)/employer/hour), `law::sightings` (suspects × guards), the
Arrest/Patrol considerations (scan every suspect/warrant), `demography::relocate` / `swap_partner`, world creation
(~N²), `remove_agent`'s citizens loop. Linear floor after the fixes projects ~1,400 tps at 20k.

Hard limits: `TilePos {x: u8, y: u8}` (~375 sites, `map.rs:99` asserts ≤ 256), building `capacity: u8` (Jail at
160), u8 levers (`guard_count`, `immigration_per_week`, `sanitation_count`), 9 corp CSV slots, `MAX_DISTRICTS` 8,
full-map flow fields (5.6 MB each at 3,000 × 1,875; the 512 cache would be ~2.9 GB).

## Phases

### S1: kill the N² paths (now; disjoint from the El crew fix)
- Port the probe's profiler (`prof.rs`, feature- or env-gated, zero cost off) and the byte-identical `stat_work`
  `staff_of` change.
- `law::sightings`: a spatial hash of guards (or suspects); same results.
- Arrest / Patrol considerations: candidates from an index (near the guard, or warrants by district), not a city
  scan.
- `relocate` / `swap_partner`: free beds indexed by area; attempts count toward the daily limit.
- Co-worker pairs: sample k co-workers per agent-hour (a config key), not all pairs. Behaviour change: measure.
- World creation (job assignment, housing pre-roll) and `remove_agent`: no city scans.
- Acceptance: 2k gates green; every byte-identical change proven byte-identical; 20k probe tps (probe scratch map)
  ≥ 4× the baseline.

### S2: widen the types (when nothing else is in flight: it touches everything)
- `TilePos` to u16, building `capacity` to u16, the u8 levers to u16/u32, the corp CSV slots and `MAX_DISTRICTS`
  dynamic or raised. Save version bump.
- Acceptance: the 2k city byte-identical (CSV and events) on seeds 42-47; all gates green.

### S3: big maps (two coders in parallel after S2)
- S3a, paths: hierarchical / sector pathfinding (or a bounded flow-field cache keyed by sector); memory bounded
  at 3,000 × 1,875. The 2k city's paths may differ only if proven equal-length.
  **Done (2026-10-12):** below `[exec] flow_field_max_tiles` (131,072 = the 64 MiB `flow_field_budget_mb` over
  512 fields) the flow fields stay, now also capped by bytes: the 2k city is byte-identical (CSV and events, seeds
  42-47 × 120). Above it a Full walk asks `exec::bigpath::find` once for an exact A* path and carries it in its
  `Goto` (saved; re-found when its next tile is no longer a step away or walkable): integer costs, ALT landmarks
  (8, measured on the tiles without rubble, rebuilt only when a tile changes, so a pure function of the tiles)
  over 7 × Manhattan, connected-component labels for an O(1) "unreachable". Why not sectors: only Full agents
  (`max_full` 50) path, so queries are few (~230 a day on the bench) and an exact search is cheap; HPA*'s portals
  are inexact unless every boundary tile is one, and an exact property test against Dijkstra was the bar.
  Manhattan alone flooded on long walks (a Door or Ground tile anywhere on the path puts every road tile of the
  box under the true cost: 260k tiles expanded on a cross-island query); the landmarks cut the bench's total path
  time 1,680 → 341 ms over 3 days. Memory at 3,000 × 1,875: search scratch 51 MB (per thread), labels 22 MB,
  landmarks + tile snapshot 96 MB, no flow fields. The El's per-tile stop shortlist (`transit::Net::near`, 90 MB
  at that size, rebuilt on every load) is now a per-16 × 16-cell candidate list (620 KB with 486 stops, O(cells ×
  stops) to build). The commute estimates stay Manhattan, as at 2k; the bench's Full walks run 2.5 % longer than
  Manhattan (doors, the river's bridges). Bench: `tests/big_island.rs` (ignored), nine copies of the 2k city on a
  3,000 × 1,875 street grid with a river; 6,000 residents, 3 days: 1,300-1,500 ticks/s after day 0, path queries
  p50 8 µs / p99 16 ms, peak working set 380 MB; main's flow fields on the same run: 340-400 ticks/s and 746 MB of
  fields after 3 days (139 of the 512 allowed). 18,000 residents: 630 ticks/s, peak 452 MB; `max_full` 500: ~2,300
  queries a day, p99 4.6 ms, 570-630 ticks/s.
- S3b, the map spec: a `MapSource` behind the current text map; a deterministic in-repo generator for a 20k
  stand-in (districts, Blocks, Farms/Fabs, Markets, civic, Hideouts, rail lines as `[[transit.lines]]`,
  employers sized to the population); the TheCityGraphics adapter (`blocks.json`, `districts.json`,
  `elevated.json`, per-lot `sim_layout.toml`: lots → buildings with floors, flats → beds, shop units →
  workplaces, the rail layer → El lines). The adapter is a CLI subcommand that writes the sim's own map files;
  a downsampled snapshot of one import is committed as a fixture so tests never need the graphics repo.

#### S3b status (2026-10-12)
- **The map spec** (`citysim/src/map.rs`, all optional, old maps parse as before): an `rle` header (run-length
  grid rows); `D <zone> <x_from> <x_to> <name>` district rows and a `districts` grid (any district shape);
  `L <x>,<y> ... <name>` El lines; `#` comments. A map's districts and lines replace the config's
  (`Map::apply_to` in `World::new`). `MapSource`: `[world] map` is a file (default `map.txt`) or
  `gen:<residents>[:<seed>]`. A Home's `floors` field now deals a household a floor at creation (5 residents a
  floor; no Home on the shipped map has the field, so the 2k city is unchanged).
- **Saves** (SAVE_VERSION 13): the map's grids are run-length strings (61 KB for the 2k map against ~550 KB;
  1.3 MB at 3,000 x 1,875 against ~62 MB). A v12 save loads and runs on byte-identically.
- **The generator** (`citysim::mapgen`, `citysim-cli gen-map --residents N --seed S --out FILE`): one borough
  per 2,000 residents, each a copy of the shipped layout (`assets/gen_map.py`), so employers and food stay near
  the homes; the one-per-city buildings and the shipped eight districts (by name: seeding passes look them up)
  in the central borough; the other boroughs split by zone within `MAX_DISTRICTS`; two El lines per borough row,
  two per column. At 2,000 residents and seed 0 it is `assets/map.txt` exactly (a test). The committed stand-in
  is `assets/maps/gen20k.txt` (1,280 x 376, 26 districts, 14 lines, 367 KB). Per 1,000 residents: 200 Blocks,
  6 Vat Farms, 1.5 Markets, 1.5 Bars, 1 Hideout, 1 Security Office, 80 Lots; one Precinct, Civic Hall, Reserve
  Depot and Recycler per city.
- **The adapter** (`citysim-cli import-graphics`): docs/IMPORT_GRAPHICS.md (the mapping, the gaps, asks for
  their side). The fixture is `assets/maps/isle_m_k4.txt` (the whole island at 8 m tiles, 750 x 469).
- **Smoke, seed 42, on main + S3a (`ca3b244`)** (`run --population N --scaled --map ...`; ticks/s on this box alone; before S3a: gen20k ~1,000 / ~450 late, the import 1,100-2,600):

  | map | residents | days | employed | homeless | starved (cum.) | Murders | corps | ticks/s |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | gen20k | 20,000 | d0 / d10 / d20 / d29 | 17.0 % / 17.1 % / 17.1 % / 21.2 % | 80 / 16 / 4,798 / 12,314 | 0 / 0 / 0 / 3,617 | 100 in 30 days | 9 / 9 / 9 / 9 | ~1,300 (d0-6), ~1,000 (d8-13), ~780 (d15-20), ~450-550 (d24-29) |
  | isle_m_k4 | 5,000 | d0 / d9 | 35.3 % / 36.9 % | 15 / 6 | 0 / 0 | 10 in 10 days | 9 / 9 | 3,000-4,900 |
  | (2k reference, map.txt) | 2,000 | d0 / d29 | 42.9 % / 51.4 % | 80 / 196 | 0 / 0 | 12 in 30 days | 9 | ~4,100-6,500 (six seeds in parallel) |

  **The 20k city collapses from day 15** (evictions: RentShort 25k, Evicted 16k; homeless 12k; 3,617 starved
  with 350k food on the Market shelves: no coins to buy it). The map is not the binding constraint; the config's
  absolute keys are (S4's list, measured here): `[corps] farms / markets / offices` (12 of 120 Farms, 3 of 30
  Markets and Bars, 2 of 20 Security Offices owned at day 0; `Config::scaled_to` leaves them by design);
  the World market's export demand (food exported 362/day at 20k against 376 at 2k: the outside money
  doesn't scale, wages 19.5k/day against 4.4k); `market_initial` is per Market and `scaled_to` scales it too
  (444k food on the shelves at day 0 against 5.8k: 100x with 10x the Markets); the seeded venues, Labs, Fabs and
  Clinics are fixed counts (3 Clubs, 2 Fabs, 1 Lab at 20k). On a map without the shipped district names
  (the import) five seeding passes find nothing: no Garage, Clinic, Lab, Feed, Fixer or Unplugged chapel
  (`assets::seed_sellers`, `virt::SEED_LABS`, `news`, `contracts::SEEDED_FIXERS`, `creeds`); a name-agnostic
  rule (by zone, or by the map's own tag) is S4's. The import otherwise runs: both El lines carry riders
  (fares 3,066/day), the piers feed migrants, no starvation in 10 days.
- **Manhattan commute estimates vs the real walk** (S3a's open item, measured, not fixed): 2,000 random
  Block-to-workplace pairs, `bigpath::find` steps over Manhattan: `isle_m_k4` p50 1.01 / p90 1.05 / max 1.37;
  the 2 m import p50 1.00 / p90 1.03 / max 1.58; `gen20k` max 1.28. Small today (the import's hills read as
  Water and its street grid rarely detours); docs/IMPORT_GRAPHICS.md.

### S4: per-capita config and the 20k profile
- Convert the absolute keys that assume 2,000 (probe list: `[lod] max_full/max_coarse`, plan budgets, stock caps,
  gang homes/names, sweeps, migrants, riots, money floors/caps, `max_hunts`, `cook_target`, ...) to per-1,000 or
  per-building forms; the 2k values must reproduce today's numbers exactly.
- A 20k gate profile (`core_20k`: 30 days, sanity bounds scaled per capita, its own tps floor) and a 20k
  shadow/behaviour read. A sane 20k city: employment, hunger, homeless, deaths in the 2k city's per-capita band
  (findings, not tuned asserts, per the no-fine-tuning doctrine).

### S5: the Ambient tier
- Flats as seeds + occupancy; per-district aggregate demand feeding the markets, rents, labour pool and street
  density; deterministic materialise / dematerialise (hire, immigration, a story touch, the player's circle);
  the parity-style check that a materialised resident looks like one who was simulated all along.

## Doctrine reminders
Data first; failing-first tests; determinism; never retune an earlier milestone's knobs (convert them per
capita so 2k reproduces exactly); don't regenerate the stat table casually (check Murders vs main).
