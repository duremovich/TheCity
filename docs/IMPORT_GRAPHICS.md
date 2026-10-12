# Importing TheCityGraphics (Scale S3b, S3c)

`D:\TheCityGraphics` (Dylan's graphics repo, **read only**: the adapter never writes there) generates the big
island: 6 x 3.75 km on 2 m tiles, 8 districts, 837 blocks, 14,516 lots (`spec/city/SIM_NOTES.md`). Its output
(`assets/city/isle_m/out/`) is gitignored on their side and still evolving, so the sim never reads it at run or
test time. An adapter writes the sim's own map format once; the committed result is what tests and runs read.

```
citysim-cli import-graphics --src D:/TheCityGraphics/assets/city/isle_m/out --out assets/maps/isle_m_k4.txt \
    --downsample 4 --residents 5000 [--crop x0,y0,x1,y1]
```

- `--downsample k`: a sim tile is k x k of theirs (k = 1: their 2 m tiles; 4: 8 m tiles).
- `--residents N` (default 20,000): the employers are sized for N residents with the generator's mix per 1,000,
  and (S3c) the Homes' household slots are dealt for N (below).
- `--crop`: a window in their metres (x east, y north), for small test maps.
- It prints a report: which inputs it read, what it built (kinds, kinds by their lots' use, the mix per 1,000
  beside the generator's), what it dropped and what it could not map.
- A missing file or field fails with the file, the record and the field (`lots.json lots[0] (l0): missing field
  "rect_m"`, `sim/lots_sim.json lots.l3: missing field "use"`), never a panic. Optional inputs (`terrain.json`,
  `street/tram.json`, `street/names.json`, a lot's `sim_layout.toml`) are warnings when absent. An unknown sim
  export `schema` (anything but `city-sim/0.1`) is a warning, and the file is read anyway; an unknown `use`,
  `ground` or `source` value is a counted warning (an unknown use reads as vacant). The written map is parsed by
  the sim's own loader before the command returns.

**The sim export (S3c).** Their `out/sim/` (SIM_NOTES "The sim export", schema `city-sim/0.1`, 2026-10-12) answers
asks 1-5 and 8 below. The adapter prefers it file by file and says which inputs it used (`input:` lines in the
report, and a `# inputs:` comment in the map):

| file | read | without it (the S3b fallback) |
| --- | --- | --- |
| `sim/lots_sim.json` | per lot: `floors`, `use`, `ground`, `flats`, `shop_units`, `doors_m` (`at`, `out`), `source`, `civic` | `lots.json` floor ranges (a seeded pick) and `bldg/<lot>/sim_layout.toml` |
| `sim/terrain_sim.json` | the 2 m class grid: `~` sea, `u` urban, `m` mountain | every tile off the blocks and ways is Water |
| `sim/rail.json` | each line's name, track (`pts`, `u`) and stations (`stops`: `name`, `u`) | `elevated.json` rail elements, named `El <id>` |

**The committed snapshot**: `assets/maps/isle_m_k4.txt` (the whole island at k = 4, 750 x 469 tiles, 213 KB,
`--residents 5000`, their seed 19 as of 2026-10-12 with the sim export). Until S3c it was imported for 20,000
residents (4x the employers a 5,000 run needs, 27,073 Home slots). `citysim/tests/mapgen.rs::
test_graphics_import_fixture_parses` reads it; nothing in the test suite reads the graphics repo. Regenerate it
with the command above when their city changes (and expect the test's counts to move).

## The map format it writes

The text map (`citysim/src/map.rs`), with Scale S3b's and S3c's optional additions (older maps parse as they did;
the shipped map and the generator write none of S3c's, so the 2k city is byte-identical):

- header `"<w> <h> rle"`: every grid row run-length encoded (`7.=` is `.......=`);
- `D <zone> <x_from> <x_to> <name...>`: a district row; the map's rows replace the config's `[districts]`
  (`Map::apply_to`, at `World::new`);
- `districts` then `h` rows of ids (`a`-`z` 0-25, `A`-`F` 26-31): every tile's district, in place of the rows'
  zone-and-x-range rule (their districts are unions of superblocks, not x ranges);
- `L <x>,<y> <x>,<y> ... <name...>`: an El line; the map's lines replace `[[transit.lines]]`;
- `# ...` comments in the building section;
- `B <Kind> <x> <y> <w> <h> <tier> <floors>`: the eighth field (floors) existed since Jobs and room J6. **S3b:** a
  Home of F floors is dealt F households at world creation (5 residents a floor). A Home with no floors field is
  one household, as before.
- **S3c:** the tile `m`, **Mountain** (`TileKind::Mountain`): walkable, 1.5 a step to the pathfinders (15 tenths
  in `exec::bigpath`, 1.5 in the flow fields; the walk itself takes a Ground step's time), never built on (no map
  building may hold one; `BuildHome` wants Ground), and not a street tile (`districts::rebuild`: no litter,
  hang-out or squat spot lands on the hills).
- **S3c:** `B ... <floors> flats=<n> storeys=<n>`: optional `key=value` fields after the floors field, carried on
  `MapBuilding` (`flats`, `storeys`, saved when present) and read by nothing yet: `flats` is the source city's
  flat count in a Home (their estimate), for S5's Ambient tier; `storeys` its storeys (the members' mean). An
  unknown key fails the load.

## Mapping table

| Their data | Sim | How |
| --- | --- | --- |
| frame: metres, x east, y north, 2 m tiles | tiles, x east, y south | `tx = (x - x0) / s`, `ty = (y1 - y) / s`, `s = 2 m x k` |
| `terrain.json` `summary.size_m` | map size | `size / s` (the blocks' and ways' bounding box when absent) |
| `districts.json` `rows` (id, name, zone) | `D` lines | name `"D1 The Sump"`; zone Sump → U, Civic → C, Mid → M, Spire → S, Vats → V (unknown: M, a warning) |
| `districts.json` `superblocks` (rect, district) | the districts grid and the zone grid | superblock tiles seed their district; a district with no superblock (D7) seeds from its blocks; every other tile (streets, sea, hills) takes the nearest seed's (BFS); the zone is the district's |
| **S3c** `sim/terrain_sim.json` `rows` | Water, Ground, **Mountain** | each sim tile the most common class of its k x k samples (ties urban, then mountain): `~` Water, `u` Ground, `m` Mountain |
| superblocks, `blocks.json` `open`, `greens_m`, `leftover_m`, `voids_m` | Ground | rounded rects; S3c: never over Mountain (a superblock's rect is a bounding box, often up the hill) |
| `blocks.json` blocks | Ground | rounded rects, over anything |
| everything else (S3b, no terrain) | Water | impassable |
| `roads.json` `ways` (rect, any `cls`) | Road | rounded edges, at least one tile across (a 2 m alley keeps a tile at any k) |
| no road on the map's edge (an island) | 4 carved piers | a Road from the nearest road straight to each map edge, so `Map::edge_roads` (migrants) is not empty |
| `blocks.json` block `parts_m` (platforms) | building rects | each part is cut along its frontage at lot edges into pieces of at least 3 tiles, then shrunk off road tiles (the edge with the largest share of road first) |
| a part thinner than 3 tiles (the Sump's 12-16 m lots past 4 m tiles) | the whole block | one part over the block's rear alley (alley tiles built over) |
| a Vats part | uncut (S3b); **S3c:** cut only between two lots each big enough for a Farm on its own | at 2 m tiles most vat farms are (all 120 Farms land on `vat_farm` lots); at 8 m none are, and the godowns stay whole |
| `lots.json` lot (block, rect, `front`) | which cut it belongs to; the door side | a part's front is its lots' most common front |
| **S3c** `lots_sim.json` `floors` | `storeys=` | the members' mean; Homes' household slots come from flats (below), workplaces keep one floor |
| **S3c** `lots_sim.json` `use`, `ground`, `civic` | the cut's use | a member's named civic site, else its members' most common use; "workshop" when half its members' ground floor is one |
| **S3c** `lots_sim.json` `doors_m[].out` | the door | the wall tile (not a corner) whose outside tile is an `out` tile and Road, else the road-facing wall tile nearest any `out` tile, else the S3b side search; a door whose outside tile a later whole-block cut built over moves to the nearest wall tile still facing open Road |
| **S3c** `lots_sim.json` `flats` | Home floor slots, `flats=` | `ceil(N / 5)` households dealt over the Homes by their flats (largest remainder), at least one slot a Home; `World::new` deals households by stride over the slots, so a tower holds its flats' share |
| S3b `lots.json` `floors` `[lo, hi]` + `seed` | floors | `lo + seed % (hi - lo + 1)` (the district's range, not the lot's massing) |
| S3b `bldg/<lot>/sim_layout.toml` | floors, flats, shop units, the door | storeys (tier not `roof`); units with a `living` room; units with a `shop`; `door_tiles` on one-lot cuts facing N or S |
| kinds | `B` lines | the mapping below (S3c) or the generator's mix by district (S3b, no export) |
| **S3c** `sim/rail.json` `lines` | `L` line named after it ("Terrace Line") | the track's points to tiles (Ramer-Douglas-Peucker, 1.5 tiles), each station's nearest track point (by `u`) kept as a waypoint (a waypoint is a stop); the station names go in a `#` comment; the service is not read |
| S3b `elevated.json` element `kind = "rail"` | `L` line "El \<id\>" | each `station` element's nearest track point kept |
| `street/tram.json` `lines` | `L` line "Tram \<street\>" ("Tram Tai Ning Road") | the loop from `u = 0` out to the far `terminus`, its stops kept as waypoints; the street's name from `street/names.json` (the way id without it) |
| `lots.json` `district` | (unused) | the cut takes the district under its centre on the grid |

### Kinds from the export (S3c)

The counts stay the generator's per 1,000 residents (`--residents`); the export decides where. Picks within a
stage go to the preferred zones first, then spread by stride over the city (or the largest first).

| `use` (`ground`) | Sim kind | Rule |
| --- | --- | --- |
| `civic` `city_hall` | Hall (Civic Hall) | the named lot's cut |
| `civic` `precinct_<district>` | Jail (the Precinct) for the one in a Civic district (D2), **Security Office** for the other five | one city Precinct (the sim has one Jail); the district precincts count toward the Security Offices' 1 per 1,000 |
| `civic` `crematorium` | Cemetery (the Recycler) | the named lot's cut |
| `civic` `depot` | Warehouse (the Reserve Depot) | the closest kind: the city's one food store |
| a named site the cuts or the window lost | the S3b rule | largest Civic / Vats cut, with a warning |
| `vat_farm` (`works`) | Farm (Vat Farm) | 6 per 1,000, largest first: `vat_farm` cuts with 60 interior tiles, then any industrial, then any cut nobody lives in, then any; the rest Lots |
| `factory`, `godown` (`works`) | Security Office, then Lot | Security Offices to 1 per 1,000 (factories, then godowns, Vats first); the rest Lots (the industrial plots corps and the seeding passes build on) |
| `shop_house` (`workshop`) | Hideout, then Block | 1 per 1,000, Sump first |
| `shop_house` (`shop`) | Street Market, then Block | 1.5 per 1,000, Sump then Mid first |
| `commercial` (`shop`) | Bar, then Lot (or Block if it has flats) | 1.5 per 1,000, Mid (the Neon Quarter) then Sump first |
| `residential` (`lobby`) | Block (Home) | always |
| `vacant` (`none`) | Lot | always |
| (any industrial, civic or vacant leftover) | Lot | always |
| Lots to 80 per 1,000 | Lot | after the leftovers: commercial cuts without flats, commercial, shop-houses, by stride; never past the generator's 80 Lots to 200 Blocks (a coarse import has too few cuts for both; the Blocks come first) |
| the rest | Block (Home) | tier by zone: Sump/Vats 0, Mid/Civic 1, Spire 2 |

Every Home carries `flats=` and every building `storeys=`. `shop_units` steers nothing yet (Markets and Bars
follow `use` and `ground`; ask 3's unit count is in the report).

### Measured (their seed 19 with the sim export, 2026-10-12)

| k | `--residents` | tiles | file | cuts | Homes (slots) | Farms (on `vat_farm`) | Markets / Bars | Hideouts | Security Offices | Lots | short |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 20,000 | 3,000 x 1,875 | 1.15 MB | 8,765 | 6,941 (8,908) | 120 (120) | 30 / 30 | 20 | 20 | 1,600 | none |
| 2 | 20,000 | 1,500 x 938 | 536 KB | 6,104 | 4,280 (6,280) | 120 (71) | 30 / 30 | 20 | 20 | 1,600 | none |
| 4 | 20,000 | 750 x 469 | 209 KB | 2,788 | 1,869 (4,543) | 68 (16) | 30 / 30 | 20 | 20 | 747 | Farm 68 of 120, Lot 747 of 1,600 |
| 4 | 5,000 | 750 x 469 | 213 KB | 2,788 | 2,328 (2,388) | 30 (16) | 8 / 8 | 5 | 5 | 400 | none (the committed snapshot) |

The mix per 1,000 residents (the report's `mix` lines; the generator's: Blocks 200, Vat Farms 6, Markets 1.5, Bars
1.5, Hideouts 1, Security Offices 1, Lots 80):

| map | Blocks | Vat Farms | Markets | Bars | Hideouts | Security Offices | Lots |
| --- | --- | --- | --- | --- | --- | --- | --- |
| k = 1 at 20,000 | 347 | 6.0 | 1.5 | 1.5 | 1.0 | 1.0 | 80 |
| k = 2 at 20,000 | 214 | 6.0 | 1.5 | 1.5 | 1.0 | 1.0 | 80 |
| k = 4 at 20,000 | 93 | 3.4 | 1.5 | 1.5 | 1.0 | 1.0 | 37 |
| k = 4 at 5,000 (the snapshot) | 466 | 6.0 | 1.6 | 1.6 | 1.0 | 1.0 | 80 |

So k = 2 is the closest fit for 20,000 (Blocks 214 per 1,000 against the generator's 200); k = 1 carries 1.7x the
Blocks (a household slot each, the rest of the 4,000 households by flats) and k = 4 is too coarse for 20,000
(Farms need 60 interior tiles: few godowns are that big at 8 m). The employers are the generator's mix exactly
at k = 1, 2 and k = 4 at 5,000; whether that mix is enough for N people is S4's question (below: it is not, on
this map or the generator's).

Every k imports both lines (the Terrace Line with its 5 stations Ming Yip West, Ming Yip East, Sun Lam, Kam Lam,
Lok Lam; the tram along Tai Ning Road out to its far terminus) and 7 districts. Doors at k = 1: 8,078 on their
`out` tile, 685 on the nearest road-facing wall tile, 2 by the side search (vacant lots have no door); at k = 4:
2,083, 705, 0, and 34 moved off a rear alley a whole-block cut built over. The flats total 808,220 (their
estimate; 778,852 in the snapshot's Homes); the hills are 146,485 of the snapshot's 351,750 tiles (42 %).

## What is not mapped (and what the sim would need from their side)

These are listed by the adapter's report too. Suggestions are for Dylan, who may want to add fields there.

1. ~~Per-lot massing floors~~: `lots_sim.json` `floors` (S3c), written as `storeys=`.
2. ~~Per-lot use and the civic sites~~: `use`, `ground`, `civic` (S3c; the mapping above).
3. ~~Flats and shop units per lot~~: `flats` deals the Homes' households and is kept as `flats=`. `shop_units` is
   counted but places nothing: Markets and Bars follow `use` and `ground`, one building per cut (the sim has no
   per-unit shops yet).
4. ~~Street doors in the city frame~~: `doors_m` (S3c). Only a laid-out lot lists more than one; a cut keeps one
   door (the sim's buildings have one).
5. ~~Land, sea and mountain~~: `terrain_sim.json` (S3c): the hills are walkable Mountain.
6. **Height.** Platforms' z, retaining walls, `under_deck` lots, pylons, `ladder` (266) and `stair_boulevard` (23)
   ways (stairs read as flat Road), the escalator, the funicular and the 23 footbridges (`walkway`) are not mapped:
   the sim's ground is one flat layer, and the hills cost a flat 1.5 a step whatever their slope. Per-tile move
   costs from `terrain_height.png` and level portals (stairs, lifts) are later work on the sim's side.
7. **Vehicles.** The expressway, its ramps, `roads.json` nodes/edges and `street/traffic.json` lanes: the sim does
   not simulate vehicles on roads yet (SIM_NOTES: "geometry is the pipeline's, state is the sim's").
8. ~~Rail service~~: `rail.json` (S3c): the line's name and stations are read; its service (240 s headway, 480 s
   at night, 05:30-01:00, 14 m/s, 30 s dwell, 4 trains) is not: the sim's `[transit]` headway, dwell and ride
   speed run every line (Dylan: the sim owns its timetable). The tram's 150 s headway and timetable (`tt`) are
   not read either.
9. **D7 The Stack and D8 Shelter.** D8 is not generated (no superblock: not a sim district; the import handles its
   absence). D7 has no superblock of its own (`d7_superblock` names `sb02_01`, which `superblocks` lists under
   D1); the adapter gives a district with no superblock its blocks' tiles, so D7 is a district of one block, its
   one 200 x 120 m lot cut like any other.
10. **Room-level data.** Rooms, portals, `Use` slots and room `capacity` in `sim_layout.toml` are not placed: a
    sim building is one rect with a door, its capacity from `[buildings]` x floors.
11. **Open space.** Parks (`greens_m`), sports grounds, nullahs and the cemetery voids are Ground (or Mountain
    where the terrain says so); the Recycler is the crematorium lot.

**New asks for their side (S3c, all optional):**

- **A `use` for the Mid-Levels' podiums.** D4 / D5 / D7 lots are all `residential`; a podium's shops (and a
  tower's lobby) would give the Mid-Levels Markets and Bars of their own (today every Market lands on a Sump or
  Neon shop-house).
- **A slope or grade class on mountain tiles** (or the height in `terrain_sim.json` at 2 m) if the hills should cost
  more where they are steep; the sim reads one flat Mountain cost today.
- **The precinct's rank.** The sim has one Precinct (its Jail) and reads the other precincts as Security Offices;
  a `civic` name for the central one (`police_hq`?) would replace the adapter's "the Civic district's" rule.
- **Their `use` vocabulary as a list in the schema** (`legend` like `terrain_sim.json`'s), so a new use is a
  schema change the adapter can name rather than an unknown value.

## Findings from the smoke runs (S3c)

`citysim-cli run --seed 42 --scaled --report --events`, main `41f0213` + S3c, this box alone.

| map | residents | days | employed d0 / d10 / d20 / d29 | homeless d0 / d10 / d20 / d29 | starved (cum.) | Murders | corps | El rides a day | fares a day | ticks/s |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `isle_m_k4` (S3c snapshot, 5k mix) | 5,000 | 30 | 24.1 / 25.2 / 27.7 / 29.5 % | 35 / 24 / 484 / 3,050 | 0 to d26, 14 by d29 | 32 | 9 | 490-645 (~95-170 on Sundays) | 2,636 → 1,338 | 1,400-4,400 (~2,000 mean) |
| the S3b snapshot (20k mix), same binary | 5,000 | 30 | 35.3 / 36.8 / 38.3 / 39.7 % | 15 / 11 / 159 / 820 | 0 | 35 | 9 | 880-1,140 | 3,250 → 1,982 | 1,300-3,100 |
| S3c import at k = 4 for 20k (20k mix) | 5,000 | 30 | 35.3 / 35.9 / 37.7 / 39.5 % | 30 / 5 / 394 / 2,572 | 2 | 47 | 9 | 640-820 | 2,825 → 1,914 | 1,600-3,900 |
| S3c import at k = 1 (20k mix, 3,000 x 1,875) | 20,000 | 15 | 16.7 / 16.9 / - / - (d14: 17.0 %) | 35 / 10 / d14: 308 | 0 | 51 in 15 days | 9 | 1,220-1,360 | 10,059 → 4,045 | 360-640 (15 days in 61 s) |

What it says:

- **The money and ownership collapse arrives at 5,000** (`RentShort` 7,551, `Evicted` 3,188 in 30 days; homeless
  3,050 at day 29, starvation from day 27). It is S4's (the absolute `[corps]` counts, the World market, the seeded
  employers by district name); no config was changed here. At 20,000 on the 2 m import it starts on schedule
  (`RentShort` 12,127 in 15 days, homeless 308 at day 14, the generator city's day-15 cliff).
- **Fewer jobs at the 5k mix.** The S3c snapshot carries the generator's mix for 5,000 (30 Farms, 8 Markets, 8
  Bars, 5 Security Offices): 25-30 % employed. The S3b snapshot carried 4x those employers at 5,000 (68 / 30 / 30 /
  20) and the same 2k-city mix per 1,000 on the 2k map gives 43-51 %: the per-1,000 mix alone is not enough
  while the name-keyed seeding passes (Garage, Clinic, Lab, Feed, Fixer, the chapel) find nothing on the import
  (S4).
- **A poorer city.** The S3c import (its 20k-mix map, so the same employers as S3b) collapses faster than S3b's:
  homeless 2,572 against 820 at day 29, rent paid 1,882 against 1,357 a day at day 14. S3b dealt households by
  seeded floor counts, and the Spire's `[2, 150]` floor range put 32 % of them in tier-2 Homes (`coins_by_tier`
  wallets); by flats the Spire holds 7-12 % (the snapshot: tier 0 28 %, tier 1 60 %, tier 2 12 %; the 2k city: 50 /
  35 / 15). That is their city's shape (the Mid-Levels' towers hold most flats), not a bug.
- **The El carries riders on both lines** (Terrace Line and Tram Tai Ning Road): 490-645 rides a day at 5,000 on
  the snapshot (fewer than S3b's 880-1,140: a quarter of the employers to ride to), 1,220-1,360 at 20,000. Fares
  fall as the money goes.
- Ticks/s: ~2,000 at 5,000 on the snapshot; 360-640 at 20,000 on the 3,000 x 1,875 map (the generator city: ~1,300
  early, ~450-550 late).

**Commute estimates vs the walk on the hills** (S3b's probe, re-run on the S3c maps: 2,000 random Block-to-
workplace door pairs, `exec::bigpath::find`'s steps over the Manhattan estimate, `transit::commute_ticks`' basis;
the walk's time is its steps, Mountain or not):

| map | p50 | p90 | p99 | max | unreachable | paths over Mountain |
| --- | --- | --- | --- | --- | --- | --- |
| `isle_m_k4` (S3c) | 1.01 | 1.05 | 1.14 | 1.46 | 0 | 265 of 2,000 (948 Mountain steps) |
| k = 1 (S3c) | 1.00 | 1.03 | 1.11 | 2.13 | 0 | 211 of 2,000 (2,962 Mountain steps) |
| `isle_m_k4` (S3b, hills as Water) | 1.01 | 1.05 | 1.13 | 1.37 | 0 | - |
| k = 1 (S3b) | 1.00 | 1.03 | 1.14 | 1.58 | 0 | - |

Walkable hills hardly move the estimate: the built city is a coastal strip whose street grid already runs the
short way, and a Mountain step costs 1.5 so few paths take one (those that do are short cuts across a spur). The
tail grows (k = 1 max 2.13: a 39-tile pair walking 83 steps, 0.1 % of pairs over 1.5); Manhattan stays the
commute estimate.

See docs/SCALE_20K.md § S3b for the 20k generated city.
