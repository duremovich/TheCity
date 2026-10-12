# Importing TheCityGraphics (Scale S3b)

`D:\TheCityGraphics` (Dylan's graphics repo, **read only**: the adapter never writes there) generates the big
island: 6 x 3.75 km on 2 m tiles, 8 districts, 837 blocks, 14,516 lots (`spec/city/SIM_NOTES.md`). Its output
(`assets/city/isle_m/out/`) is gitignored on their side and still evolving, so the sim never reads it at run or
test time. An adapter writes the sim's own map format once; the committed result is what tests and runs read.

```
citysim-cli import-graphics --src D:/TheCityGraphics/assets/city/isle_m/out --out assets/maps/isle_m_k4.txt \
    --downsample 4 [--residents 20000] [--crop x0,y0,x1,y1]
```

- `--downsample k`: a sim tile is k x k of theirs (k = 1: their 2 m tiles; 4: 8 m tiles).
- `--residents N`: the employers are sized for N residents with the generator's mix per 1,000 (below).
- `--crop`: a window in their metres (x east, y north), for small test maps.
- It prints a report: what it read, what it built, what it dropped and what it could not map.
- A missing file or field fails with the file, the record and the field (`lots.json lots[0] (l0): missing field
  "rect_m"`), never a panic. Optional inputs (`terrain.json`, `street/tram.json`, a lot's `sim_layout.toml`) are
  warnings when absent. The written map is parsed by the sim's own loader before the command returns.

**The committed snapshot**: `assets/maps/isle_m_k4.txt` (the whole island at k = 4, 750 x 469 tiles, 145 KB,
seed 19 of their pipeline as of 2026-10-12). `citysim/tests/mapgen.rs::test_graphics_import_fixture_parses` reads
it; nothing in the test suite reads the graphics repo. Regenerate it with the command above when their city
changes (and expect the test's counts to move).

## The map format it writes

The text map (`citysim/src/map.rs`), with Scale S3b's optional additions (older maps parse as they did):

- header `"<w> <h> rle"`: every grid row run-length encoded (`7.=` is `.......=`);
- `D <zone> <x_from> <x_to> <name...>`: a district row; the map's rows replace the config's `[districts]`
  (`Map::apply_to`, at `World::new`);
- `districts` then `h` rows of ids (`a`-`z` 0-25, `A`-`F` 26-31): every tile's district, in place of the rows'
  zone-and-x-range rule (their districts are unions of superblocks, not x ranges);
- `L <x>,<y> <x>,<y> ... <name...>`: an El line; the map's lines replace `[[transit.lines]]`;
- `# ...` comments in the building section;
- `B <Kind> <x> <y> <w> <h> <tier> <floors>`: unchanged, the eighth field (floors) existed since Jobs and room J6.
  **New with S3b:** a Home of F floors is dealt F households at world creation (5 residents a floor), so a
  multi-storey block holds its flats' people. A Home with no floors field is one household, as before.

## Mapping table

| Their data | Sim | How |
| --- | --- | --- |
| frame: metres, x east, y north, 2 m tiles | tiles, x east, y south | `tx = (x - x0) / s`, `ty = (y1 - y) / s`, `s = 2 m x k` |
| `terrain.json` `summary.size_m` | map size | `size / s` (the blocks' and ways' bounding box when absent) |
| `districts.json` `rows` (id, name, zone) | `D` lines | name `"D1 The Sump"`; zone Sump → U, Civic → C, Mid → M, Spire → S, Vats → V (unknown: M, a warning) |
| `districts.json` `superblocks` (rect, district) | the districts grid and the zone grid | superblock tiles seed their district; a district with no superblock (D7) seeds from its blocks; every other tile (streets, sea) takes the nearest seed's (BFS); the zone is the district's |
| superblocks, `blocks.json` blocks, `open`, `greens_m`, `leftover_m`, `voids_m` | Ground | rounded rects |
| everything else (sea, mountain) | Water | impassable |
| `roads.json` `ways` (rect, any `cls`) | Road | rounded edges, at least one tile across (a 2 m alley keeps a tile at any k) |
| no road on the map's edge (an island) | 4 carved piers | a Road from the nearest road straight to each map edge, so `Map::edge_roads` (migrants) is not empty |
| `blocks.json` block `parts_m` (platforms) | building rects | each part is cut along its frontage at lot edges into pieces of at least 3 tiles, then shrunk off road tiles (the edge with the largest share of road first) |
| a part thinner than 3 tiles (the Sump's 12-16 m lots past 4 m tiles) | the whole block | one part over the block's rear alley (alley tiles built over) |
| a Vats part | uncut | a godown stays whole (a Vat Farm needs 60 interior tiles) |
| `lots.json` lot (block, rect, `front`) | which cut it belongs to; the door side | a part's front is its lots' most common front |
| lot `floors` `[lo, hi]` + `seed` | floors | `lo + seed % (hi - lo + 1)` (the field is the district's range, not the lot's massing) |
| `bldg/<lot>/sim_layout.toml` `[[floors]]` (tier not `roof`) | floors | storeys; a cut's floors are its lots' mean |
| `sim_layout.toml` rooms of kind `living` (distinct `unit`) | flats (counted) | reported; a Home holds 5 residents a floor (about 2.1 flats of ~2.4) |
| `sim_layout.toml` rooms of kind `shop` (distinct `unit`) | shop units | a cut with shop units (or a non-residential `[building] kind`) is preferred for Markets and Bars |
| `sim_layout.toml` `[building] door_tiles` | the door | on a one-lot cut at k = 1 facing N or S; else the front side's middle, outwards, then the other sides; the outside tile must be Road (else the cut is dropped) |
| kinds | `B` lines | the generator's mix per 1,000 residents (`--residents`): Vat Farm 6 (largest Vats cuts with 60+ interior tiles, then any), Security Office 1 (Vats), Hideout 1 (Sump), Market 1.5 and Bar 1.5 (shop cuts in Mid/Sump, by stride), Lot 80 (by stride over the city); the Precinct and Civic Hall (the largest Civic cuts), the Reserve Depot (largest Vats), the Recycler (Vats); the rest are Blocks (tier by zone: Sump/Vats 0, Mid/Civic 1, Spire 2) except in the Vats and Civic, where they are Lots |
| `elevated.json` element `kind = "rail"` | `L` line "El rail" | the track's points to tiles, simplified (Ramer-Douglas-Peucker, 1.5 tiles), each `station` element's nearest track point kept (a waypoint is a stop) |
| `street/tram.json` `lines` | `L` line "Tram \<way\>" | the loop from `u = 0` out to the far `terminus`, its stops kept as waypoints |
| `lots.json` `district` | (unused) | the cut takes the district under its centre on the grid |

Measured on their seed 19 (2026-10-12), `--residents 20000`:

| k | tiles | m a tile | file | cuts kept | lots dropped | Homes (floor slots) | Farms | notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 3,000 x 1,875 | 2 | 941 KB | 8,322 | 0 | 6,494 (124,765) | 120 | |
| 2 | 1,500 x 938 | 4 | 407 KB | 5,928 | 0 | 4,100 (86,611) | 120 | |
| 4 | 750 x 469 | 8 | 145 KB | 2,787 | 407 (100 cuts under 3 tiles) | 1,007 (27,073) | 68 of 120 | the committed snapshot |

Every k imports both lines (the El with its 5 station elements as waypoints; the tram out to its far terminus) and 7 districts. The 502 lots with a `sim_layout.toml` (one superblock, sb06_02) hold 11,081 flats and 375 shop units.

## What is not mapped (and what the sim would need from their side)

These are listed by the adapter's report too. Suggestions are for Dylan, who may want to add fields there.

1. **Per-lot massing floors.** `lots.json` `floors` is the district's `[lo, hi]` range on every lot (the massing
   pass knows each lot's storeys: median 9, p90 31). The adapter draws a seeded floor count in the range.
   *Ask:* a `massing_floors` (or `floors` as one number) per lot.
2. **Per-lot use.** Only the 502 generated tenant stacks say what they are (`[building] kind = "residential"`),
   and the Vats and Civic lots carry no use at all. The adapter places every workplace by the per-capita mix and
   by district. *Ask:* a `use` per lot (residential / shop-house / commercial / industrial / civic, and for the
   Vats godown / factory / vat farm), and the civic sites by name (precinct, city hall, depot, crematorium).
3. **Flats and shop units per lot without a layout.** Known only for the 502 laid-out lots. *Ask:* `flats` and
   `shop_units` per lot from the massing (even estimated), so a Block's households and a street's shops follow
   their city.
4. **Street doors in the city frame.** `door_tiles` are in the building frame (street to the south, rotated by
   the yaw in `kitplaced_<block>.json`); the adapter only uses them on single-lot cuts facing N or S. *Ask:* the
   street door tiles per lot in city metres in `lots.json`.
5. **Land, sea and mountain.** Only the PNG masks (`terrain_masks.png`) say where the shore and the urban limit
   are; the adapter reads every tile outside a block, way or superblock as Water, so the country park is
   impassable. *Ask:* the masks as run-length rows or polygons in `terrain.json`, if the sim should walk the hills.
6. **Height.** Platforms' z, retaining walls, `under_deck` lots, pylons, `ladder` and `stair_boulevard` ways
   (stairs read as flat Road), the escalator, the funicular and the 23 footbridges (`walkway`) are not mapped: the
   sim's ground is one flat layer. The sim would need per-tile move costs (stairs slower) and level portals
   (the building-interiors note: stairs and lifts as layer portals) before these mean anything.
7. **Vehicles.** The expressway, its ramps, `roads.json` nodes/edges and `street/traffic.json` lanes: the sim does
   not simulate vehicles on roads yet (SIM_NOTES: "geometry is the pipeline's, state is the sim's").
8. **Rail service.** `elevated.json` has the track and 5 station elements (`layers.json` stats say 7) but no line name, platform side,
   headway or timetable; the sim's `[transit]` headway applies to every line. The tram's 150 s headway, 17 cars
   and timetable (`tt`) are not read either. *Ask:* a `rail.json` like `tram.json` (line, stations by name and
   track distance, headway).
9. **D7 The Stack and D8 Shelter.** D8 is not generated yet (no superblock: not a sim district). D7 has no
   superblock of its own (`d7_superblock` names `sb02_01`, which `superblocks` lists under D1); the adapter gives
   a district with no superblock its blocks' tiles, so D7 is a district of one block, its one 200 x 120 m lot cut
   like any other (Blocks of 10-14 floors). *Ask:* the Walled City's own generator (many small buildings and
   internal lanes) will want its lots in `lots.json` and its lanes in `roads.json`; nothing else is needed.
10. **Room-level data.** Rooms, portals, `Use` slots and room `capacity` in `sim_layout.toml` are not placed: a
    sim building is one rect with a door, its capacity from `[buildings]` x floors. A capacity per building from
    the rooms' `capacity` sum is a possible later read.
11. **Open space.** Parks (`greens_m`), sports grounds, nullahs and the cemetery voids are Ground; the Recycler is
    placed in the Vats, not at the cemetery void.

## Findings from the smoke run

`citysim-cli run --days 10 --seed 42 --population 5000 --scaled --map assets/maps/isle_m_k4.txt` (2026-10-12, main + S3a):
employed 35 % → 37-39 % (the 2k city: 43 % → 47 % over the same days), homeless 15 → 6, no starvation, 10 Murders,
9 corps, 3,000-4,900 ticks/s (1,100-2,600 before S3a); both lines carry riders (3,066 fares a day). What broke:

- **Seeding by district name.** Five seeding passes look districts up by the shipped names ("Sump Central",
  "Mid East", ...): on the import there is no Garage, Clinic, Lab, Feed, Fixer or Unplugged chapel. The venues,
  Fabs, the Mission and the Camp seed by zone and land. A name-agnostic rule is Scale S4's (docs/SCALE_20K.md).
- **Vat Farms at 8 m tiles**: 68 of 120 (a Farm needs 60 interior tiles; few godowns are that big at k = 4).
  At k = 2 all 120 fit.
- **Tall Blocks**: 1,007 Blocks hold 27,073 floor slots (135k residents at 5 a floor); 5,000 residents are dealt
  by stride over the slots, so most low Sump walk-ups start empty.

- **Commute estimates are Manhattan** (`transit::commute_ticks`, hiring, relocation; S3a's open finding). Measured
  on 2,000 random Block-to-workplace door pairs against S3a's exact walk (`exec::bigpath::find`, steps): on
  `isle_m_k4` path / Manhattan p50 1.01, p90 1.05, p99 1.13, max 1.37, none unreachable; on the 2 m import
  (k = 1) p50 1.00, p90 1.03, p99 1.14, max 1.58 (0.1 % over 1.5); on `gen20k` max 1.28. The undercount is small
  here because the importer reads the hills and the sea alike as Water and the built city is one gridded coastal
  strip whose street grid rarely needs a detour. It will bite if
  the hills become walkable or the Shelter (D8, piers across the cove) lands.

See docs/SCALE_20K.md § S3b for the 20k generated city.
