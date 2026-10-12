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

#### S4 status (2026-10-12)
**The rule.** `[scale]` (config.toml) names the reference city every absolute key was tuned on: the shipped
map at 2,000 residents. Two passes scale from it, and every ratio is 1 in the shipped city, which stays
byte-identical (CSV without `ticks_per_sec`, and events, seeds 42-47 x 120, checked on the final code):
- `Config::scaled_to(n)` (per person, `f = n / 2,000`): the old keys as before; the S4 keys **up only** (a city
  under 2,000 keeps the 2,000 city's value, as before S4: the small test cities were calibrated on it).
- `Config::fit_to_map(&map)` (`World::new`, after the map's districts and lines are in): per building, per gang,
  per corp and per district, by the map's count of the kind over `[scale]`'s (ratios under 1 read 1);
  `[scale] enabled = false` in `v1_profile`.

| key | old (absolute) | rule | 2k | 20k (gen) |
| --- | --- | --- | --- | --- |
| `[world] market_initial`, `[economy] restock_floor/batch`, `price_ref_stock` | per Market, but `scaled_to` scaled them too (the double scaling: 444k food at day 0) | `x f / (Markets / 3)` | 1,500 / 1,200 / 1,200 / 1,500 | same per Market |
| `[corps]` rows (`farms`, `markets`, `offices`, `blocks`, `treasury_initial`, `megacorp`, `niches`) | 9 rows, Farms/Markets/Offices absolute (12 of 120 Farms owned) | the roster dealt `k = round(map employers / 17)` times ("Nutrix 2", ...), each column re-dealt to `round(sum x ratio)` (largest remainder), treasuries `/ k`; new `[corps] template` | 9 corps | 90 corps own 120/30/20 |
| `[corps] bar_owner_count`, `bar_owner_coins` | 2, 200 (x f) | `x Bars/3`, `/ (Bars/3)` (per owner) | 2, 200 | 20, 200 |
| the corp Bar (`ownership::seed`) | the roster's one food lead | one per roster copy | 1 | 10 |
| `[gangs] max_members` | 60 (x f by `scaled_to`: 600) | per gang: `/ (Hideouts/2)` | 60 | 60 |
| `[gangs] max_gangs` | 4 | `x Hideouts/2` | 4 | 40 |
| gang names | `[gangs] names`, then "Gang N" | past the names, generated ("The Neon Dogs", 144 distinct) | as before | 20 Hideout gangs |
| `[buildings] jail / hall / cemetery / warehouse` capacity and stock_cap | absolute | per capita over the map's count of the kind | 160 / 20 / 8 / 6, 20,000 | 1,600 / 200 / 80 / 60, 200,000 |
| `[riots] riot_max`, `riot_promote_max`; `[law] gang_landlord_homes`, `sweep_full` | per district | `x` residents per district / 250 | 40, 20, 20, 10 | 123, 62, 62, 31 |
| `[riots] max_active_riots`, `[law] max_crackdowns` | city-wide | `x districts / 8` | 2, 2 | 7, 7 |
| `[street] shanty_lots_max` | 20 of 160 Lots | `x Lots/160` | 20 | 200 |
| `[street] seed_derelict_blocks` | 16 | per resident (they empty occupied Blocks; a tall-Block map has many Homes per resident) | 16 | 160 |
| `[street] seed_hotels`, `city_absorb_floor` | 2, 5,000 | `x f` | 2, 5,000 | 20, 50,000 |
| `[jobs] seed_venues`; `[charity] seed` | absolute | `x f` | 3/3/10/2/2/1/2; 1 | x10 |
| `[world_market] food/parts/data cap`, `cap_wages`, `treasury_ref`; `[export] treasury_ref` | 350/40/12, [0,100,30], 50k; 20k | `x f` (money enters at the 2k per-capita rate) | as before | x10 |
| `[demography] migrants_max`, `beds_ref`, `offers_max` | 20, 60, 6 | `x f` | as before | 200, 600, 60 |
| `[hunt] max_hunts`; `[contracts] max_open`; `[missions] max_missions`; `[hitmarket] max_open`, `posts_per_day`, `corp_year_cap`; `[fviolence] day_cap` | city-wide | `x f` | 16; 256; 12; 16, 3, 2; 3/12/20/2 | x10 |
| `[budget] band`, `works_max`; `[treasury] civic_pace` | 30k/60k, 120; 5 | `x f` | as before | x10 |
| `[life] relocate_per_day` | 8 (a floor under `relocate_per_1000`) | `x f` | 8 | 80 (never binds) |
| `[lod] max_full`, `max_coarse`, `private_quota` | 50, 150, 12 | `x f` (measured, below) | as before | 500, 1,500, 120 |
| `[brain] plan_budget_per_tick`, `plan_expansion_budget_per_tick`, `path_budget_per_tick` | 12, 600, 20 | `x` (bodied tier / 200) | as before | 120, 6,000, 200 |
| `law::reconcile_guards`, `districts::reconcile_sanitation` | 5 moves a day (code) | `Config::per_capita_count(5)` | 5 | 50 |
| `law_brain::REPORT_LOG_CAP`, `REPORT_PLACES_CAP` | 128, 512 rings the crackdown pressure reads over 7 days (code) | `per_capita_count` | 128, 512 | 1,280, 5,120 |
| `PlayerCommand` Set{GuardCount,Sanitation,ImmigrationPerWeek} ceilings | 60, 60, 30 | `Config::lever_ceiling` (x f) | as before | 600, 600, 300 |
| seeded Garages/Clinics, Labs, corp Feeds, Fixers, Unplugged chapels | one set | `Config::copies()` = round(f) sets (below) | 1 | 10 |

Verified per holder and left absolute: `[law] patrol_beat_homes` (a loop walkable in a shift), `[data] store_cap`
(per store), `[buildings] market.stock_cap` (per Market), `[corps] hoard_heat`, `outside_treasury_initial`,
`estate_buyer_cap/heir_cap`, `[gangs] corp_raid_cap` (per corp, and the corps are copies), `[stims]
cook_target` (per gang), `stim_restock_floor` and `[living] reserve_release_batch` (per Market), `[news]
stories_per_day` and `[fixers] offers_per_day` (per Feed, per Fixer), `[transit] staff_per_line` (per line),
`[treasury] police_per_jail_bed` (the Jail scales), `[law] crackdown_reports_per_1000`, `[corps] residents_per_*`,
`[fixers] fixers_per_pop`, `[life] relocate_per_1000` (already per capita).

**More corps, not bigger ones** (generated 20k, seeds 42-44, day 60): the roster dealt ten times read employed
47.7-49.4 % and homeless 246-260 per 1,000; nine rows owning ten times the buildings, with every per-corp coin key
(`hoard_heat`, `lobby_min_treasury`, the estate caps, the outside treasuries, `corp_raid_cap`) ten times over,
read 45.1-46.9 % and 278-284. Copies also keep an exec per ~2,000 residents and a bankruptcy's share of the economy.

**The bodied tiers per capita.** Both at 50/150 at 20k: gang members 0.8 and arrests 46 per 1,000 against the 2k
city's 46 and 373 (a gang recruits only through its bodies; the gangs die) at 435-784 ticks/s. Coarse per capita:
gangs back (28-30) at 117-235 ticks/s. Full per capita too: +0.4-1.4 points employed, -6-7 homeless per 1,000, the
same ticks/s. The cost is a body's goal scoring over city-wide lists (Unwind's venues, Squat's derelicts: the
profile's top sites), not the residents.

**Seeding by role** (`districts::Seat`, `seat_district`): a row is a zone and a position west to east among its
districts (by centroid); on the shipped map Sump 0.0 / 0.5 / 0.9, Mid 0.0 / 0.75 and the one Spire, Civic and
Vats resolve to the old names, so the 2k picks are the same. Copy `j` of `n` takes the fraction `pos + j/n`; past
the zone's district count the list widens to the Mid's (the generator folds the outer boroughs' Spire, Civic and
Vats into Mid). The corp rows read their template (`World::corps_of_template`, `corp_template`): Zetatech's Labs
and Garages go to every Zetatech copy, the tech trees seed by template. Converted: `assets::seed_sellers`,
`virt::SEED_LABS`, `news::SEED_FEEDS` (the Civic Wire stays one per city; "Nutrix 2 Now" per copy),
`contracts::SEEDED_FIXERS`, `creeds::seed_unplugged` ("The Unplugged 2", each its own Purist gang),
`jobs::seed_owner` (the Fabs to the template copies in turn; the Lounge's heir), `charity::seed` (one Mission per
Sump district by Blocks), `street::seed_hotels` (rounds over the Sump districts). Seeded at day 0: the import (5k,
arbitrary district names) 5 Garages, 8 Ripperdocs, 12 Labs, 4 Feeds, 3 Fixers, 2 chapels (main: none); gen20k 20,
30, 40, 11, 10, 10.

**`[lod] coworker_crew`** stays 0. 2k (seeds 42-47 x 120, summed), 32 vs 0: Friend edges 370,786 vs 382,120
(-3.0 %, inside the 53-70k per-seed spread; S1's -43 % predates the El crew cap), employed +0.5 %, homeless -3 %,
Murders +6 %, Marriages +1.6 %, ticks/s +11 %. 20k (42-43 x 30): Friend +8 %, ticks/s +4 %, employment flat. Small
at 2k, but not byte-identical, and the 20k profile doesn't need it for throughput (the co-worker pairs are ~2 % of a
20k tick now); recommended for a later default flip only alongside a regenerated table.

**20k vs 2k per 1,000 residents** (final code; gen20k `run --population 20000 --scaled`, seeds 42 / 43 / 44;
the 2k city seeds 42 / 43 / 44):

| | day | employed % | homeless | starved (cum.) | Murders (cum.) | gang members | jailed | corps alive | Treasury / resident | ticks/s |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2k | 30 | 51.4 / 53.3 / 51.6 | 98 / 87 / 88 | 0 | 6.0 / 6.1 / 4.0 | 46 | 42 | 9/9 | 28.0 / 26.8 / 24.6 | (4,000+) |
| 20k | 30 | 48.5 / 48.2 / 48.6 | 97 / 103 / 97 | 0 / 0.05 / 0 | 5.8 / 6.6 / 6.0 | - | - | 87/90 (core_20k) | 28.3 / 26.4 / 26.2 | 86-110 (six runs on the box) |
| 2k | 60 | 56.0 / 59.2 / 58.9 | 161 / 144 / 150 | 0 | 16.8 / 14.3 / 13.2 | 67 / 60 / - | 54 / 52 / - | 9/9 (1, 0, 1 bankrupt) | 33.4 / 37.6 / 29.7 | |
| 20k | 60 | 51.6 / 51.5 / 49.8 | 199 / 204 / 201 | 0.15 | 16.0 / 14.8 / 16.5 | 40 / 40 / 40 | 43 / 31 / 32 | 90 rows, 13-16 bankrupt | 35.7 / 34.3 / 34.3 | 82-95 |
| 5k import | 30 | 47.4 / 46.1 / 47.3 | 129 / 142 / 139 | 0 | 7.7 / 7.5 / 7.1 | 26 / 27 / 21 | 29 / 33 / 28 | 27 rows, 1-2 bankrupt | 30.5 / 29.4 / 32.8 | 1,030-1,090 |

Before S4 (S3b's smoke, seed 42): gen20k 21 % employed, 616 homeless and 182 starved per 1,000 by day 29; the
new 5k fixture (import-v2's run on main) 25-30 % employed, ~610 homeless per 1,000 and starvation from day 27.

**What still diverges per capita, and why (a finding, not chased: coordinator ruling 2026-10-12).** By day 30 the
20k city employs ~3 points fewer (J24 0.497 of free adults vs ~0.53 at 2k), by day 60 ~6 points, with ~50 more
homeless per 1,000. The gap grows with size: generated 4k 50.0 / 53.2 % on day 30 (2k-like), 10k 49.8-49.9 %, 20k
48.2-48.6 %. Day 0 matches role for role (42.8 % vs 42.9 %). Days 1-29, per 1,000 (2k seeds 42 / 43 vs 20k 42 / 43),
hires later than day 0, exits (laid off, fired, quit), net:
- Guards: +14.0 / +9.5 hired, 6.5 / 1.5 out at 2k; +6.2 / +7.0, 10.3 / 10.9 out at 20k (net 29-30 vs 17.5): the
  Security copies lay off private guards on revenue ("a shift past full staff"). Contracts spread evenly over the
  twenty Security corps (11-23 each); the demand per corp is lower (smaller gangs, fewer raids to secure against:
  gang members 40 vs 60-67 per 1,000 by day 60).
- Vat Techs: net 149-150 vs 166-186 (later hires 51-56 vs 62-85).
- Clerks: +0.6-0.9 later vs +6.0-6.5; Night Stockers +0.7-0.9 vs +2.0-3.5 (already at 4k: Clerks +1.8).
- Fab Techs: 8-9 exits vs 0 (the per-capita Fabs sell Parts at the per-capita cap, and lay off on revenue).
- Bartenders: net 10-11 vs 8.5-9.5 (matched once the corp Bar went per copy).
- Sanitation: net 29.6-29.9 vs 31.5-32.5 (was 7.5 before the reconcilers' daily five went per capita).
Also: 13-16 of 90 corps bankrupt by day 60 (the 2k city 0-1 of 9); Vagrancy per capita several times the 2k city's
(the sweeps run per district, and the districts hold 3x the residents).

**Throughput.** 20k reads 155-173 ticks/s in `core_20k` (two runs in parallel) and 82-130 with six runs on the box,
falling over the run (210 on day 1, ~90 on day 60). The bodies' goal scoring is the cost (above). `TPS_FLOOR_20K` is
120, printed.

**The gate:** `core_20k` (docs/TESTING.md): 20k seed 42 x 30 days twice in parallel; per-capita collapse bounds,
an employment collapse floor of 0.40 (J24's 0.5 printed), determinism, the coin identity. Passes on the final code.

### S5: the Ambient tier
- Flats as seeds + occupancy; per-district aggregate demand feeding the markets, rents, labour pool and street
  density; deterministic materialise / dematerialise (hire, immigration, a story touch, the player's circle);
  the parity-style check that a materialised resident looks like one who was simulated all along.

#### S5 design (2026-10-12, written before the code)

**What is Ambient.** Every flat of a map Home beyond the simulated tier's household slots: on a map with `flats=`
(the import) a Home's Ambient flats are `flats - floors` (its slots are the sim's beds, used or not: the k = 1
island holds 808k flats over 8,908 slots, so ~799k Ambient flats); on a map without `flats=` it is
`round(floors x [ambient] flats_per_slot)`, 0 by default, so the shipped 2k map has no Ambient tier and its city
is byte-identical (CSV and events; the save gains nothing: the state is skipped when empty). An Ambient flat is
one household of adults (children are not modelled for the tier yet; a later step).

**What a flat stores: nothing.** A flat `(building, k)` is a pure function of `(world seed, building index, k)`
(`econ::hash_unit`'s mixer seeding a private ChaCha stream; never the world stream): occupied with
`[ambient] occupancy` (a hash < p), one or two adults (`couple_p`), and on materialisation everything about
them that is a person rather than a state (below). Stored per Home with Ambient flats, not per flat: the
building, its flats, its occupied flats, its Ambient adults left and a cursor (the next flat to materialise;
flats are taken in index order, so the cursor is the whole "touched" set). 24 bytes a Home: ~170 KB for the
island's ~7k Homes, **0.2 bytes a flat** (target 16). Occupied counts are hashed once at `World::new` (~1 ms per
100k flats). A flat in a derelict or demolished Home reads vacant (the people left with the building); the
aggregates and materialisation skip it.

**Districts carry aggregates** (daily at midnight, `systems::ambient::daily`, O(Ambient Homes)): per district the
Ambient households and adults; the labour pool (Ambient adults x the district's simulated unemployment); food
demand (adults x the simulated city's meals per resident yesterday); rent demand (households x the mean rent of
the district's Homes); crime (adults x the district's simulated crimes per resident over 7 days); street density
(adults x the district's simulated street density per resident, the addendum-13 ledger's input). These are
estimates read by the labour pool (it gates the hires) and offered to the density ledger
(`ambient::street_density`); they move no coin and no food.

**Money: a labour pool, not a consumer (yet), and why.** Doctrine: no safety net, no magic wages; money enters
via outside demand. The Ambient tier is part of the outside ledger: its households earn and spend inside the World
account's economy (the island beyond the 20k is "outside" to the simulated core, as the export market is). The
simulated economy meets it two ways only: (1) **labour**: a materialised adult crosses in with savings from the
World account (`Flow::Migrant`, `ownership::cross_in`, as an immigrant's savings do), so the coin identity holds
to the coin; (2) later, **consumption**: an Ambient household buying the Vats' food at a city Market is export
demand (World money for city goods) and belongs in the World market's appetite and caps, not in a second channel.
Wiring 800k consumers into 20k residents' Markets would swamp the shelves and needs that calibration; it is an open
question for Dylan (below), not done here. Ambient households pay no rent to city landlords for the same reason (a
city landlord's rent from World money is an export of housing).

**Materialise** (`ambient::materialise(world, home)` -> the household's adults, Statistical): the flat at the
Home's cursor (the next occupied one). The person comes from the flat's stream: sex, name (a shared surname for a
couple), age (`[world] age_min..age_max`), personality, the three v1 skills, hacking, the social skills, body,
schooling (the Home's class) and health history; the same flat gives the same person whatever was materialised
before it. The **state** comes from a donor: a simulated adult of the same class picked from the flat's stream out
of yesterday's class lists (`ambient::daily`): needs, mood, fun and the wallet's size (the coins cross in from the
World; a World short of coins pays what it holds); the donor's contacts are copied in count, kind and affinity onto
the nearest residents of the person's own Home and district (so a materialised resident has the social reach of
one who lived there). No food is copied (goods are never created). The Home gains a bed per adult
(`capacity + 1`), the person enters it, `EventKind::Materialised` is logged, the counters move.

**Who materialises.** (a) **The labour pool**: `job_search`'s vacancies that no simulated adult filled are offered
to the Ambient Home nearest the workplace (by door, any distance), bounded by
`[ambient] hires_per_day` per 2,000 residents (`Config::per_capita_count`); the household's first adult is hired
(the role's skill floored as a J21 offer's, a guard lawful). (b) **Immigration first**: with Ambient flats left, a
J20 offer materialises the household nearest the workplace instead of spawning a migrant at the edge, and a J19
pull-week arrival (and the dole city's weekly immigrant) materialises a household picked by the day's stream,
weighted by each Home's Ambient adults. Ambient arrivals count in their own columns, not `immigrants`. (c) **Story
and the player's circle**: `ambient::touch(world, home)` (a stub over `materialise`: the hook M18's view and a story
reaching an unsimulated flat will call; nothing calls it yet).

**Dematerialise: later.** Folding a long-idle Statistical agent back into its flat needs a compact record (name,
personality, skills, wallet) kept per returned flat and every index that names the entity cleaned (`remove_agent`
does, as for an emigrant; the wallet would cross out to the World). Not built in S5: the simulated tier only grows
by the bounded hires and the weekly arrivals.

**Determinism and the save.** No world-stream draw anywhere in the tier: the flat's own stream, the day's keyed
stream for the pull pick. The state (`World::ambient`: the Ambient Homes, the district aggregates, the class
donor lists, the counters) is saved (serde default, skipped when empty: a 2k save is unchanged), SAVE_VERSION 14;
a save mid-run resumes byte for byte. The CSV gains an Ambient group only on a world with an Ambient tier (the 2k
CSV is byte-identical, as the slot groups only grow past their floors).

**The check** (`tests/ambient.rs`): a city with an Ambient tier materialises a cohort on day 20; on day 30 its
needs, wallet, contacts and employment are printed against the born-simulated residents of the same class and
asserted in a loose band; materialising flats in two orders gives the same people; the 2k city has none.

#### S5 status (2026-10-12)
Built as designed above (`citysim/src/ambient.rs` the state, `citysim/src/systems/ambient.rs` the logic; hooks in
`World::new`, `demography::run` (the daily pass before the job search, the labour pool after it, the arrivals),
`systems::stats` (the CSV group); `[ambient]` in config.toml; `run --ambient-flats R`, `--no-ambient`,
`--ambient-hire-far`, `--mem`). Not built: dematerialise; Ambient consumption and rent (design above); children in
Ambient households. A map Home the seeding passes turned into something else (the Unplugged's Chapel on a derelict
Block) or built on a Lot has no Ambient flats (`seed` matches map Homes by their rect).

**The 2k city is byte-identical** to `da07983` (CSV without `ticks_per_sec`, and events, seeds 42-47 x 120), its
header and its saves carry nothing Ambient.

**Measured** (seed 42, 30 days, `run --population 20000 --scaled`, five runs in parallel on the box; ticks/s
are those runs'):

| map | Ambient | flats (households, adults) | materialised (a day) | hires | arrivals | employed d29 | homeless d29 | starved | Murders | peak WS | ticks/s d10-29 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| k = 1 import (3,000 x 1,875) | off | - | - | - | - | 8,175 | 2,270 | 4 | 270 | 672 MB | 89 (79-101) |
| k = 1 import | on | 782,884 (720,199, 900,629) | 2 in 30 days (0.1) | 1 | 0 | 8,197 | 2,239 | 5 | 300 | 672 MB | 94 (79-112) |
| k = 1 import | on, `hire_far` | 782,884 (719,862, 900,205) | 426 (14.2; 128 at most) | 338 | 0 | 8,211 | 2,372 | 2 | 297 | 675 MB | 96 (85-119) |
| gen20k | off | - | - | - | - | 9,647 | 1,925 | 0 | 224 | 500 MB | 109 (93-135) |
| gen20k | on, 50 a slot | 199,450 (183,483, 229,388) | 1 in 30 days | 0 | 1 | 9,586 | 1,927 | 0 | 238 | 504 MB | 109 (91-134) |

- **Memory: nothing measurable.** 24 bytes per Ambient Home (6.9k on the import: ~170 KB, 0.2 bytes a flat); the
  peak working set moves within the run-to-run noise (672 / 672 / 675 MB). The occupancy hash at `World::new` (783k
  flats) is not visible in the day-1 reading.
- **The labour pool hardly fires at 20k**: the city has thousands of unemployed simulated adults and ~10 open
  places a day, so a simulated candidate is almost always found (2 materialisations in 30 days on the import, 1 on
  gen20k). **Immigration never runs** in either 20k city: the J19 pull is 0 (the expected wage is under the outside
  wage) and no J20 offer stands its `offer_days`. The binding constraint at 20k is jobs, not labour.
- **`hire_far`** (an Ambient household within `[life] commute_hire_ticks` before a simulated adult beyond it) fires
  ~14 a day (128 on day 0's staffing round, then 0-15): +426 residents in 30 days, employed at day 29 +14
  against the default (+36 against off), homeless +133: the hires near work are new mouths, and the far
  simulated adults they displaced stay jobless. Off by default (the decision is Dylan's, below).
- ticks/s: the tier costs nothing measurable (O(Ambient Homes) a day, plus O(Homes) per materialisation).
- One materialisation is a butterfly: the 20k runs with the tier on diverge from off after it (Murders 300 vs 270
  on the import, 238 vs 224 on gen20k: run-to-run spread, not a mechanism).

**The parity-style check** (`tests/ambient.rs::test_materialised_look_like_born_residents`, the shipped map with
4 Ambient flats a slot, seed 42): 99 adults from 80 Homes materialised on day 20 at 09:00, by class:

| | n | hunger | energy | mood | wallet (median) | contacts (median) | employed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| d20 Street, materialised | 99 | 0.79 | 0.82 | +0.03 | 3 | 54 | 0.00 |
| d20 Street, born | 1,098 | 0.75 | 0.78 | -0.04 | 1 | 54 | 0.20 |
| d30 Corp, materialised | 21 | 0.77 | 0.83 | +0.08 | 18 | 84 | 1.00 |
| d30 Corp, born | 829 | 0.76 | 0.79 | +0.04 | 11 | 76 | 0.99 |
| d30 Street, materialised | 76 | 0.72 | 0.79 | -0.07 | 1 | 80 | 0.08 |
| d30 Street, born | 983 | 0.75 | 0.77 | -0.06 | 1 | 77 | 0.20 |

98 of 99 alive on day 30; 21 found corp work in ten days (Corp class). The fresh cohort holds no job (a story touch
materialises a household, not a worker), so day-20 employment reads 0 against 0.20; by day 30 the Street cohort
reads 0.08. The contacts were 25 against 52 with `edges_max` 24 (the first cut); 96 copies the donor's reach.

## Doctrine reminders
Data first; failing-first tests; determinism; never retune an earlier milestone's knobs (convert them per
capita so 2k reproduces exactly); don't regenerate the stat table casually (check Murders vs main).
