# The city milestone: the client joins, the 100k re-import, the vertical city as pathed levels

Drafted 2026-10-30 against TheCity `1a66424` and TheCityGraphics `855d0aa` (master = `integration-bridge`). Opens
after Shadow V18 closes the pressure pass. Dylan's calls are `docs/ROADMAP_POST_M14.md` addendum 22; the view
contract, the join plan and the map check are TheCityGraphics `spec/integration/` (`SIM_VIEW_CONTRACT.md`,
`WORKSPACE_JOIN.md`, `MAP_RECONCILE.md`). The doctrine holds throughout (CLAUDE.md): no test unless a phase names
one, a phase is proven by its shadow read and, from C2 on, by the follow-cam; every number in `assets/config.toml`;
no child is ever harmed.

## Why

- **The pressure pass has its exit** (V18), and far travel recurred in every read from V12 as a map problem patched
  locally (`OPEN-221`, `OPEN-240`, `OPEN-241`'s musters 5-7 h from the members). The perf pass phase 10 measured
  the map's half: at 100k the `k = 2` island gives 1,415 of 8,000 wanted Lots, employment 0.553 (`OPEN-204`); height
  is not short (332,862 flats for 20,000 households), ground sites are.
- **The pace is the other half, and no map fixes it alone.** `[exec] move_ticks_full = 1` on 4 m tiles is 4 m a game
  minute, 0.24 km/h, 1/21 of a human walk (R2, `OPEN-215`); a 1 km leg is ~4 game hours, the El (14 m/s) is ~200x a
  walk. Every city sim pays this (a game minute is 1-3 real seconds, walks look right on screen); the answer here
  is that a life happens within a few blocks, the Hong Kong way: work, food and the evening in the tower or the next
  one. That is what floors as tenancies give: TheCityGraphics' export already holds 15,698 shop units and 4,978
  workshop grounds (`lots_sim.json`), twice the 8,000 sites 100k wants, on the same 6,104 cuts.
- **Dylan's calls:** the client joins first so every later phase is watched as well as read; the vertical city is
  pathed levels (floors as tenancies `OPEN-187`, a z, stairs and lifts as portals with travel cost, footbridges; no
  below ground); this session's agents may write TheCityGraphics on its own branches, by its HANDOFF and
  `pipeline/DECISIONS.md`.

## What exists (the anchors the design fits)

- **A building is a rect, one door, `floors`** (`components.rs` `Building`; "they cost no pathfinding: the door is
  the one portal"). `Position { tile, building, entered }` has no z. A body indoors stands at nothing but its
  building. The only mixed use is the island pass 3b's company floors: a Home `within` a workplace, same rect and
  door (`Building.within`, `World.company_homes`, 8 call sites).
- **The map** (`map.rs`): `B <Kind> x y w h tier floors flats= storeys= lot=`, `L`/`L+` El lines, `D` districts;
  `citysim-cli import-graphics` (`import_graphics.rs` `build`, `read_layout`, `read_sim_lots`, `door_lot`) cuts the
  blocks at lot edges, reads each laid-out lot's `sim_layout.toml` for floors, flats and shop units, and stops the
  Lots at the generator's 80 Lots to 200 Blocks (`import_graphics.rs:1738`).
- **Paths** (`exec/bigpath.rs`): above `[exec] flow_field_max_tiles` (both island maps) a Full walk asks one exact A*
  on tenths-of-a-step tile costs with ALT landmarks (`Landmarks::build`, `dial`) and 4-connected component labels
  (`labels`); a Coarse walk is a timed arrival (`exec::timed_goto`, `timed_walk`: Manhattan x pace) placed along its
  road way when read (`exec::walk_place`, `motion::walk_way`). Commute estimates are Manhattan
  (`transit::commute_ticks`, `transit::reach_discs`).
- **The layouts carry the levels already:** each of the 13,216 laid-out lots' `sim_layout.toml` (TheCityGraphics
  `pipeline/blender/building/navgen.py` `sim_layout`) lists `[[floors]]` (index, z, height, tier, a tower's
  `plate`), `[[rooms]]` with `floor` and `unit` (a flat or shop id), `[[portals]]` (`flight` stairs, 108,915 `lift`
  portals city-wide, `street` doors) and `[[slots]]` (Use targets, `lift_call`). The 1,300 estimated lots carry only
  `floors`, `flats`, `shop_units`. Footbridges are `elevated.json`: 23 `walkway` elements (D2 only, a 5 m deck at
  +6 m, `portals` to stairs and podiums), one `escalator` (one-way by the hour: down 06:00-10:00, up 10:00-24:00),
  4 `ramp`s, 5 `station`s; none of it is in `out/sim/` yet.

## Phases

Ten phases, one fresh Opus agent each (~300 turns; C6a and C6b are the two that may run long: say so and hand
back rather than grind). Iterate at 20k (`--population 20000 --scaled --ambient-flats 50 --map
assets/maps/isle_m_k2.txt`, seed 42 x 30); one 100k run a phase at most (`isle_m_k2_100k.txt`, 8 threads). Numbers
are before and after on the same seed, printed, never banded. **Waves:** 1 = C1 ∥ C3; 2 = C2 ∥ C4; 3 = C5; 4 = C6a;
5 = C6b; 6 = C7; 7 = C8; 8 = C9. C3 and C4 are TheCityGraphics only and serial because both regenerate its
gitignored `out/` on one disk; C1 touches both repos (the move) and nothing C3 touches.

### C1, the workspace join (TheCity + TheCityGraphics `engine/`; `WORKSPACE_JOIN.md`, `Cargo.toml`, `tools/test.py`, `tools/hooks/pre-commit`, `docs/TESTING.md`)

Move `glclient` and `cityview` to `view/glclient`, `view/cityview` and the vendored `bevy_pbr` / `bevy_render`
0.19.1 fixes to `view/vendor`, as one plain move commit naming TheCityGraphics `855d0aa` (the history stays there;
its `engine/README` points here, `lighttest` keeps its own vendor copy). The root manifest per WORKSPACE_JOIN:
members plus `default-members = ["citysim", "citysim-app", "citysim-cli"]`, `[patch.crates-io]` at the root,
cityview's `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 3` moved to the root (a member's
profile is ignored with a warning), `[profile.release.package.cityview] debug = 0` if the link hurts. The view
crates keep edition 2024 and their own `view/rustfmt.toml` (no 160-hunk reformat). `asset_root()` reads
`CITYVIEW_ASSETS`, default `<root>/../TheCityGraphics/assets`, failing at startup with the path tried; benches take
`--out`. **The change tier stays Bevy-free:** every `--workspace` and `--all` in `test.py` goes (fmt `-p citysim -p
citysim-app -p citysim-cli`; clippy, tests, `--no-run --message-format=json` binary discovery at `test.py:189`,
`:287`, `--only` at `:313` on default members), the hook takes the same `-p` list, and a new `test.py view` tier
(fmt, clippy `-D warnings`, `cargo build --release -p cityview`, the smoke: `cityview --sim <fixture> --bench one`
exits 0 and writes its PNG) runs outside change/merge/milestone. **Check the lock:** `cargo tree -p citysim` before
and after must list the same versions (resolver 2 unifies semver-compatible versions in one `Cargo.lock`; a Bevy
dependency bumping one the sim shares is pinned back with `cargo update --precise`). **Camera:** cityview built in
TheCity draws the city from a fixture `simsnap` wrote (the bridge's 20k day-1 render). **Proof:** the change tier's
wall before and after (no Bevy line in its log), the view tier's first walls into TESTING.md. Moves no sim number.
No test.

**C1 as built (2026-10-31).** As above, with these deviations: (1) **Bevy without `audio`** in both view crates
(`default-features = false`, `2d` + `3d` + `ui` + `area_light_luts`): `bevy_audio`'s `alsa-sys` and citysim-app's
macroquad (`quad-alsa-sys`) both declare `links = "alsa"`, and one lock may hold one package per `links` value, so
the join did not resolve with Bevy's defaults. The view plays no sound; a sound phase re-opens it (an audio crate
without `alsa`, or macroquad's audio off). (2) The dev profile: `opt-level = 1` by package for cityview and glclient
(`[profile.dev.package.<name>]`), not a root `[profile.dev]`, so the sim's own crates keep the dev default;
`"*" = 3` at the root as planned; `[profile.release] debug = 1` holds for the view too (the cold build is 3m07s, the
relink 27 s: no `debug = 0` needed). (3) **A formatting commit after all:** the client was never rustfmt'd, so no
`view/rustfmt.toml` makes it clean; at `max_width = 140`, small heuristics Max (the narrowest pass) it is 83 hunks
in one commit of formatting only. (4) **Clippy:** 34+ lints under rustc 1.99; `too_many_arguments`,
`type_complexity` (Bevy systems) and glclient's `needless_range_loop` (parallel arrays) are allowed crate-wide, the
rest fixed in place with behaviour unchanged. (5) Dropped in the move: cityview's own `Cargo.lock` and
`.cargo/config.toml` (`target-dir = ../lighttest/target`). (6) `--out` already existed; its default moved from the
crate's `results/` to `<TheCity>/target/cityview`. (7) The fixture is the bridge's own frame (150 KB, committed, not
generated): `view/cityview/fixtures/day1_20k_s42.simview.json`, recipe in TESTING.md. **The lock:** +337
packages, every old (name, version) kept; `cargo tree -p citysim` (and the app and CLI) identical before and after.
**TheCityGraphics** (branch `city-c1-join`): the two crates removed, `engine/README.md` points here, lighttest
reads glclient from the sibling checkout (`../../../TheCity/view/glclient`) with its own vendor copy, simsnap
unchanged (both `cargo check` green against this branch), `test_occlusion.py` runs cityview from here.

### C2, the follow-cam on a save (TheCity: `citysim/src/view.rs`, `citysim-cli` `export-view`, `view/cityview` `sim.rs`, the `SimThread`)

`citysim::view` is `simsnap`'s `frame()` moved in as structs, no I/O: `snapshot(&World, ring) -> ViewFrame`
(`SIM_VIEW_CONTRACT.md` § 2 fields; Coarse bodies placed by `exec::walk_place`, not their stale `Position`; trains
by `transit::train_at`; motions by `motion::timed_pos`). `citysim-cli export-view` is the file form
(`city-simview/0.1`) and makes the view tier's fixture. In cityview, a `SimThread` owns a `World` loaded from a
save, ticks at the view's rate and publishes frames into a two-slot buffer; `cityview --save <file> --follow
<entity index>` pins the subject, sets `SetView` to a ring around it each frame and follows it (`--rate`, default
walk-matched: **0.35 ticks/s since R2's pace**, not the contract's 0.7; see Q1). The subject is the shadow diary's
(`gang_leader_17198` is index 17198), so `citysim-cli shadow` gains `--save-at <day>` writing each window's start
save. The ring pins every agent in it (simsnap's stopgap); a ring budget outside `max_full`, promotion before
visibility and Statistical materialisation stay `OPEN-149`, the view milestone's. glclient's own tram and traffic
timetables are off when `--save` drives (traffic becomes ambient filler later). The 2D app is untouched.
**Camera:** the subject walking its day from the save, the ring's bodies moving (no Coarse stack on one tile), the
El trains where `train_at` puts them. **Proof:** three diaries from V18's picks (a commuter, a gang member, a cabbie)
followed for an hour each, a `--bench one` PNG at each quoted line's tick, paths in the report. **The phase's one
test** (`test-add: the view snapshot drops bodies silently`): `view::snapshot` on the seeded 2k world holds every
Full and Coarse body in the ring and every motion's riders aboard, ids that resolve. Moves no sim number.

**C2 as built (2026-10-31).** As above, with these deviations: (1) **Walk-matched is real time now.** `--rate` defaults
to `view::walk_matched_rate`: `[view] walk_m_s` (1.4) over the pace (`[view] tile_m` 4 x `[exec] walk_tiles_per_tick`
21 / `move_ticks_full` 1 = 84 m a tick), **1/60 ticks a second**, not R2's 0.35: since Y4 the sim walks 5 km/h, so a
walk-matched game minute is a real minute and a game day 24 real hours (contract Q1 is open again for Dylan; `--rate`
takes any value). (2) **`[view]`** in config (`tile_m`, `north_m`, `walk_m_s`, `ring_half_tiles` 48, `pin_margin_tiles`
8, `way_margin_tiles` 64), read by the view only; no `SAVE_VERSION` bump (a serde default). (3) **`ahead`** on each
body and vehicle (the Full walker's next path tiles, a timed walk's or vehicle's corners to the next tick, a train's
next place), so the view lerps along the street; the file form also carries `ring` per frame and `source.subject`.
cityview draws the newer frame's bodies lerped from the older (at a held tick exactly the newer). (4) A Coarse walker
is placed on his way (`exec::walk_route`, one road search) only when his start-to-target box grown by
`way_margin_tiles` meets the ring. (5) The ring's pins (`view::pin_ring`) are set on entry and dropped past the ring +
`pin_margin_tiles`, only the ones the view set. (6) A frame is `World.tick` after the tick ran: the diary's minute
`HH:MM` is the frame `HH:MM + 1`. (7) cityview: `--from TICK`, `--shots T1,..` (with `--bench one`, a PNG per tick,
`<tag>_d<D>_<HHMM>.png`, the sim held at each; ticks as numbers or `D:HH:MM`), the caption and "most on a tile" in
the HUD and the log; C3's `--overview --at --zoom` and its train-on-any-deck patch ported; glclient's tram and
traffic off under a `glclient::SimDrives` resource. The subject is the occlusion's player stand-in (the camera and the
cut follow it). cityview depends on `citysim` (one lock line). (8) `shadow --save-at D1,D2` (days) writes
`<out>/saves/<seed>-<tick>.ron` at each day's start, the picks pinned and `max_full` grown as in the window. (9) The
test boards three riders by hand (the 2k city's commute hour rides nothing). (10) The fixture is `export-view`'s
(147 KB, recipe in TESTING.md). **Read** (`target/c2/`, 20k seed 42 day 7, report in the session scratchpad): the
replay keeps to the diaries line by line (commuter, gang member, cabbie); the old 78-Coarse stack is gone; what reads
wrong: a street spot is one tile (123 bodies vending and wandering on one 4 m tile at 07:30, 69 wandering at 00:20,
all Full: one blob in the view); a cab rank at (847, 520) has no streamed ground (the body under the city's geometry,
the reconcile drift C5 fixes); the ring holds Statistical bodies until the hourly assignment (32-272 a frame,
`OPEN-149`); every train at a stop at 07:30 with no riders, on the old `L` lines (C5); an escorted prisoner shows no
goal or step for the walk to the Precinct.

### C3, the El on rail and the duplicate tram (TheCityGraphics branch; `pipeline/city/rail.py`, `layers.py`, `elevated.json`, `street/tram.json`, `simdata.rail`)

Route the three authored lines (`OPEN-158`: Mid-Levels 780,795 to 780,470; Hillside 561,600 to 1387,600; Vats
1387,795 to 1387,342, today `L+` from `assets/maps/isle_m_k2_lines.txt`) as track along streets through `rail.py`,
not viaducts through D4's 30-45-floor towers (MAP_RECONCILE: 28, 45 and 22 lots crossed), each with named stations
on the streets their stairs land on, and export them in `rail.json` beside the Terrace Line (`pts` with z,
`stops`). **The duplicate tram:** the sim imports "Tram Tai Ning Road" from `street/tram.json` as an El line while
glclient runs the same loop on its own timetable (WORKSPACE_JOIN blocker 5), and the Mid-Levels and Vats lines
start at the tram; find which pair duplicates, keep one tram, and record the cut in DECISIONS. If a route cannot
clear a lot without a kit regen, stop and report (a D4 regen is 4.1 h, a script left running, not this phase).
**Camera:** cityview's free overview over the three new tracks, a train on each deck. **Proof:** `simdata --check`
and the street checks green; `rail.json` lines 1 -> 4, stations 5 -> n, lots under a viaduct 0, tram loops 2 -> 1;
HANDOFF and MAP_RECONCILE refreshed. No TheCity code.

### C4, the export for 100k and for levels (TheCityGraphics branch after C3; `pipeline/city/simdata.py`, `spec/city/SIM_NOTES.md`)

The sim export at schema `city-sim/0.2`: per lot a `levels` list (floor index, z, height, its units by kind:
flats, shops, workshops, offices, the `lift` and `flight` cores and their floors) from `sim_layout.toml`, the
estimates' floors with their `ground` use and `shop_units` spread by the laid-out densities, marked `estimate`; and
`walkways_sim.json` (every walkway, the escalator, the ramps and the station stairs as portals: each end's lot or
way, its level, its length in metres, its one-way hours). HANDOFF item 2's checks (the tower `plate` floors against
what `read_layout` counts), MAP_RECONCILE refreshed against TheCity's maps, the export byte-identical per input as
its gate says. **Camera:** none (data). **Proof:** `simdata --check` green; counts printed: floors 164,750, units
by kind, lift cores, portals by kind. No TheCity code.

### C5, the 100k re-import (TheCity; `import_graphics.rs`, `assets/maps/isle_m_k2.txt`, `isle_m_k2_100k.txt`)

Re-import both maps from TheCityGraphics master after C3 and C4 (the sha and the export's seed in the map's `#
inputs`): the El lines from `rail.json` (the `--lines` file and its `L+` lines retire), the tram once, schema 0.2
read and its new fields carried, not yet used. A new world: `SAVE_VERSION` bumps, the `B` order is new, the view
fixture is regenerated. **`OPEN-204`'s call, measured** (the R5 table re-run, 100k seed 42 x 30 against phase 10's
committed column): cuts, Lots of 8,000, Farms of 600, Markets/Bars, Home slots, employed d30 and share, homeless,
legs over 120 min, workers past 3 h, El boardings a day, ticks/s days 25-30, the longest tick, build wall; and the
same at 20k seed 42 x 30. Also printed: the units the export offers per district against the sites 100k wants, the
input C6 needs. Nothing retuned (doctrine 1). **Camera:** the follow-cam on a commuter riding a new line. **Shadow:**
cabbie, office_worker, worker_friday (20k), each with an El leg quoted.

**C5 as built (2026-10-31).** As above, with these deviations: (1) **The `B` order is not new.** C3 and C4 left
`lots.json` and every 0.1 field byte-identical, so the re-import's buildings, tiles and districts are phase 10's
byte for byte (6,104 cuts at both sizes); the maps differ in their `L` lines and `#` comments only. The El lines
are new: Terrace, Hillside (24 waypoints, y 616-661), Mid-Levels (x 823, was the authored 780), Vats (x 1422, was
1387) from `rail.json` in its order, then the tram (`tram.json`, once), then the Funicular, still `L+` from
`elevated.json` (it is not in `rail.json`; with `[legs]` on an `L+` line runs like an `L`). `--lines` and
`isle_m_k2_lines.txt` are gone. (2) **`SAVE_VERSION` stays 118:** the save format did not change, and a save
carries its own map and lines, so an old save loads as the old world; nothing to bump. (3) **0.2 carried, not
mapped:** the importer reads `z_m`, `levels` (units by kind, `plate`), `cores` and `walkways_sim.json`'s portals
into its own structs, checks them against the export's `totals` (all agree: 164,750 levels, flats 332,862, shops
10,156, workshops 5,542, offices 4,030, 14,514 flight and 6,800 lift cores; 253 portals, 9,065 m) and counts them
in the report; the map format is unchanged (C6a writes `in=`/`span=`, C7 the `P` lines). (4) **The source:**
TheCityGraphics `b447d8d` (`c2df6ff` plus a DECISIONS-only commit; the export is C4's, seed 19), read by `git -C
<--src> rev-parse` into `# inputs:`. (5) **Not read:** `rail.json`'s service (headways, the rack lines' 8 m/s):
the sim's `[transit]` runs every line at one speed (nothing retuned). (6) **The camera's commuter is not a shadow
pick:** no cabbie, office_worker or worker_friday diary rides the El (below), so the follow-cam is `loader`
#19811 from a plain run's save (day 19 18:00): the tram from (575,795) at 19:44, off at Tai Ning East (1422,795)
20:16, the Vats Line aboard at 20:22 (1422,780), off at 20:28 (1422,496), at the Freight Yard 20:32.

**The numbers** (seed 42 x 30, `--scaled --ambient-flats 50`, 8 threads, one run each; before is main `872d382`
on phase 10's maps; the 100k before shared the box with builds and the shadow, the 100k after ran alone):

| | 100k before | 100k after | 20k before | 20k after |
|---|---|---|---|---|
| cuts / Lots / Farms / Markets, Bars | 6,104 / 1,415 of 8,000 / 600 / 150, 150 | same | 6,104 / 1,600 / 120 / 30, 30 | same |
| Home slots (beds) | 20,047 (100,235) | same | 5,790 (28,950) | same |
| employed d30 (of the population) | 46,237 (0.418) | 45,674 (0.413) | 11,947 (0.531) | 12,123 (0.537) |
| homeless d30 | 12,361 | 12,690 | 254 | 248 |
| legs over 120 min (30 d, `--legs-min 120`) | 12 | 30 | 4 | 4 |
| workers past 3 h (`commute_long`) | 0 | 0 | 0 | 0 |
| El boardings a day (all tiers) | 5,398 | 5,267 | 1,781 | 1,785 |
| bodied El legs in 30 days | 2,297 | 2,051 | 125 | 39 |
| ticks/s days 25-30 | 13/13/13/13/13/12 | 14/13/13/13/12/12 | 134/131/120/125/120/130 | 127/128/123/122/125/123 |
| longest tick days 25-30 | 2.96 s @00:00 | 3.12 s @00:00 | 0.26 s @21:00 | 0.28 s @21:00 |
| build wall | 58 s | 65 s | 7 s | 8 s |

The re-import moves nothing beyond noise: the buildings are the same, and the El is a Statistical carrier (20k:
1,780 of 1,785 boardings a day are `rides_stat`; ~13,000 refusals a day as `slow`), so routing the three lines
along the arterials changes only the few bodied legs (20k 125 -> 39). Against phase 10's committed column (employed
55,368, 0.553; homeless 3,431; 28 ticks/s; longest tick 0.9 s at 21:00) the 100k city is main's since: the
pressure pass (`OPEN-220`) and Y4's walking pace (`OPEN-255`: ~13 ticks/s), and midnight now holds the longest
tick (2-3 s). **The units the export offers** (C6's input; `import-graphics` prints it) against 100k's wants
(8,000 Lots, 9,100 employers at 91 per 1,000):

| district | flats | shops | workshops | offices | works lots | sites | Lots dealt at 100k | employers dealt |
|---|---|---|---|---|---|---|---|---|
| D1 The Sump | 54,049 | 4,455 | 3,241 | 0 | 0 | 7,696 | 0 | 162 |
| D2 Civic Core | 0 | 342 | 0 | 4,030 | 0 | 4,372 | 0 | 24 |
| D3 Neon Quarter | 53,182 | 1,424 | 1,260 | 0 | 0 | 2,684 | 560 | 653 |
| D4 Mid-Levels | 207,142 | 3,935 | 1,041 | 0 | 0 | 4,976 | 745 | 1,126 |
| D5 The Spire | 16,548 | 0 | 0 | 0 | 0 | 0 | 87 | 286 |
| D6 The Vats | 0 | 0 | 0 | 0 | 743 | 743 | 23 | 267 |
| D7 The Stack | 1,941 | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| all | 332,862 | 10,156 | 5,542 | 4,030 | 743 | 20,471 | 1,415 | 2,519 |

The sites are where the people are not: the Spire offers no work unit and the Vats 743 works lots for its 600
Farms; D2's 4,030 offices are C4's placeholder density. **Shadow** (20k days 21-27, cabbie x 4, office_worker x 6,
worker_friday x 4, `target/c5/shadow`): **no pick rides the El** (14 diaries, 0 `Ride` steps); the commutes are
walks of minutes (`office_worker_15375`: "Home door (770, 726) to work door (1184, 657): 483 tiles, travel 23
min", under the Hillside Line the whole way; `cabbie_12984`: 251 tiles, 12 min). That is `OPEN-254` (the El nearly
dead for bodies at a walking city), not the import. What reads wrong: in the view the rider at 20:22 is drawn on the
train's roof and at 20:25 standing on the Vats deck with no train under him.

### C6a, floors as tenancies: the map and the store (TheCity; `map.rs`, `import_graphics.rs`, `components.rs` `Building`, `world.rs`, `company.rs`, `lod.rs`, `view.rs`)

A cut stays one structure on the tile map; its uses become tenancies. **A tenancy is a `Building`** (every system
keyed on a building keeps working): the cut's main use is the **host**, each other use a `Building` with `within:
Some(host)` (3b's company floors, generalised) and a `span` of floors, its own kind, owner, rent, door (its lot's
street door from `doors_m` when it has one, else the host's) and capacity from its units. The map grows
`B ... in=<host> span=<lo>-<hi>`; the importer deals the employers, venues, Hideouts and Homes into units (a Bar on
a shop-house's ground, flats above as a Home span, an office stack), not whole cuts, so cuts go back to Homes.
**Vacant units are not entities:** a pure function of the map's unit counts (Ambient's rule), materialised by C6b's
founding. Every floor a `Building` would be 164,750 entities, 27x today's store; the target is the uses the
population needs (~14k at 100k). A household's flat is a unit of its Home span: `Household` gains its flat, and the
body indoors knows its floor (`Position.floor`, set on entry from the tenancy's span, the host's ground for the
door). Statistical mirror: the tenancy is the agent's building as before, the floor drawn from it on promotion.
**Camera:** the frame's buildings list hosts and tenancies with spans; a body indoors drawn on its floor in the
cutaway (`OPEN-180`'s floor and flat, the first half). **Shadow:** worker, cook, club_staff, dancer, super (20k):
a post and a home on named floors. **Numbers** (20k, one 100k): Buildings 6,104 -> n (hosts, tenancies), employed
share, homeless, legs over 120 min, home-to-post median, ticks/s, peak working set.

**C6a as built (2026-10-31).** As above (`systems/floors.rs` holds the store's half), with these deviations: (1)
**The host is the cut's main use by a rule:** a cut where people live (the S3c rule's Home) is a Home on its flats'
storeys (`span` the levels holding flats) and every use dealt into its units is a tenancy on it; a cut nobody lives
in (D2's office stacks, a factory) is hosted by its lowest use, the rest tenancies on it; Farms and the civic sites
stay whole cuts with no tenancies. No Home is a tenancy yet (a founded Home on a tenancy Lot would be one). (2)
**What is dealt into units:** the Security Offices (an office or a works floor first), the Hideouts (the alley's
workshop), the Markets and Bars (a shop front) and the Lots (an office stack, a works floor, a workshop, a shop),
each one unit on one storey (`span=f-f`); a use with no unit left in its stage takes a whole cut as before (the
fewest flats first), and only whole-cut Lots count against the generator's 80 Lots to 200 Blocks. A zone's share of
the Lots (`GEN_LOTS_BY_ZONE`) is dealt over its districts by their units (the sites; the Spire, with none, by its
households: whole cuts as before), the rest anywhere by the districts' household slots. (3) **A Farm no longer
takes an office stack** (`farm_desk`, a stage before the last resort): at 100k the 600 Farms had taken D2's 22
office towers whole, so D2's 4,030 offices held nothing; now D2 holds 1,223 of 100k's Lots (234 at 20k). (4) **The
map:** `span=<lo>-<hi>`, `units=<n>` (a host's work units, the input of `floors::vacant_units`: units less the
uses on the host, Ambient's rule, C6b's founding), and a tenancy's `in=<host's B line>` (0-based, file order; the
rect repeated) and `door=<x>,<y>`. A tenancy's own door is its unit's lot's street door when that `out` tile is a
wall tile of the cut facing an open road (not the host's door): 309 of 1,304 at 20k, 2,973 of 8,366 at 100k; the
rest share the host's. `Map::place_doors` finds the doors once every line is read; a Lot that hosts tenancies is
walled. A host's interior is reachable through each of its doors, so a path may cross a two-door structure (none
measured). (5) **The flat** (`Household.flat`, `(Home, index)`) is one of the Home's household slots, dealt at
`set_home` and on the first entry (kin under the roof share one; `World::new` deals them afresh once the kin are
linked); its storey (`floors::flat_floor`) deals the slots up the span from a storey fixed per Home (a hash of its
id), since at the sim's scale most Homes hold one or two slots (20k: 6,629 slots on 5,584 Homes) and every
household otherwise lived on its tower's lowest flat floor (the first shadow read so). **The floor**
(`Position.floor`): at his own Home his flat's storey, at his post one of its floors (fixed per body), anyone else
the span's lowest (a visitor, a customer); 0 on the street and in a building with no span; every
`p.building = None` sets it 0. Nothing in the sim reads it yet (C7's climb will). (6) **Statistical mirror:** the
tenancy is the agent's building as before; the floor is drawn from it on promotion (the promoted body enters
through `enter_building`, which sets it). (7) **One-building-per-cut fixes:** `build_on_lot` builds a tenancy Lot
inside its host's walls (no tile changes) and keeps a host's tenancy doors when it walls a Lot host;
`DemolishHome` with the street off (it razes the tiles) refuses a structure with tenancies; `mood`'s company-floors
crowding reads `company_homes`, not `within`; the 8 `company_homes` sites, `lod.rs` (promotion stands at the door,
then enters) and the door lookups needed nothing. (8) **The 2k city** (`mapgen`) has no spans: no floors, no flats;
the change tier's 1,471 tests ran unchanged (`test_graphics_import_20k_holds_20k` reads the hosts' `lot=` as
unique, a tenancy naming its unit's lot). (9) `SAVE_VERSION` 119; both island maps re-imported (the `B` order new);
the view fixture regenerated (185 KB; 218 of its 621 buildings are tenancies).

**The numbers** (seed 42 x 30, `--scaled --ambient-flats 50`, 8 threads, one run each; before is main `bdcdb22`;
the 20k runs shared the box with builds, the 100k after ran alone):

| | 20k before | 20k after | 100k before (C5's after) | 100k after |
|---|---|---|---|---|
| buildings (hosts with tenancies, tenancies) | 6,104 (0, 0) | 7,408 (1,036, 1,304) | 6,104 (0, 0) | 14,470 (4,265, 8,366) |
| Lots of the generator's want / Homes / Home slots | 1,600 of 1,600 / 4,280 / 5,790 | 1,600 / 5,584 / 6,629 | 1,415 of 8,000 / 3,585 / 20,047 | 8,000 of 8,000 / 5,366 / 20,088 |
| employed d30 (of the population) | 12,123 (0.537) | 11,839 (0.525) | 45,674 (0.413) | 57,458 (0.513) |
| homeless d30 | 248 | 314 | 12,690 | 2,249 |
| legs over 120 min (30 d) | 4 | 5 | 30 | 19 |
| home-to-post median (`commute_med`, d30) | 10 min | 9 min | (not printed) | 4 min |
| ticks/s days 25-30 | 113/131/127/117/136/137 | 148/137/134/130/142/133 | 14/13/13/13/12/12 | 12/12/12/11/12/11 |
| peak working set | 543 MB | 541 MB | (not printed) | 1,966 MB |

The 100k city is where the floors pay: 8,000 of 8,000 Lots stand (1,415 before) in the units of the cuts people live in and of D2's office stacks, so the employed share rises 0.413 -> 0.513 and the homeless fall 12,690 -> 2,249 (the cuts the Lots had taken went back to Homes beside them), legs over 120 min 30 -> 19, and the home-to-post median is 4 min. At 20k, where the Lots already stood, nothing moves past one run's noise (employed 0.537 -> 0.525, homeless 248 -> 314, both not diagnosed). The store grows 2.4x at 100k (14,470 buildings) for ~1 tick/s (13 -> 12; the 100k after ran alone, as C5's after did) and a 1.97 GB peak; the drop is a print for C7's perf read, not a knob. Nothing retuned.

**Camera** (`target/c6a/cam`, from the shadow's day-22 save): `c6a_croupier_d22_1715.png`, the follow-cam on
`club_staff_14197` with the occlusion's cut open on his tower: his body drawn inside it on floor 18 (z 68 m), the
storeys around him cut away; `c6a_croupier_d22_0005.png` the same tower at 00:05, the bodies indoors drawn on their
storeys up the facade. What reads wrong: the replay leaves the diary by 17:15 (the diary has him at the Gambling Den
on floor 0 at 17:12, the follow-cam asleep at home on floor 18). **Shadow** (20k days 21-23, worker, cook,
club_staff, dancer, super x 2, `target/c6a/shadow`): every pick has a flat on a named floor and every post in a unit
names its floor and host: `club_staff_14197` "Home: Block#3288 (D3 Neon Quarter), flat 1 on floor 18, floors 1-21"
and "Job: Croupier at Gambling Den#6844 (D3 Neon Quarter), floor 0 of Block#3288", his evening "d22 17:12 now at
Gambling Den#6844, floor 0 of Block#3288" and his night "d22 00:03 now at Block#3288, floor 18 (from Gambling
Den#6844, floor 0 of Block#3288)": home and post in one tower, the HK way; `cook_17235` home on floor 9 of Block#1350
(floors 2-23), the Noodle Bar on floor 0 of Block#602, 8 min; `dancer_19911` the Club on floor 1 of Bar#5748 in D2;
`super_24671` floor 16 of Block#4881, his round's Block#4882 on floor 12. What reads wrong: a visitor stands on the
Home's lowest flat floor whoever he visits (`worker_13521` at Block#697 floor 2); a Super's post is a whole Block
(his floor a hash of his id, not the floor his round is on); most Homes hold one household slot, so a flat is "flat
1" everywhere and the tower's other flats are the Ambient tier's; the worker's Corp HQ and the Vat Farm are whole
cuts with no floors.

### C6b, floors as tenancies: the economy of floors (TheCity; `founding.rs`, `ownership.rs` `Flow`, `living.rs`, rent, `company.rs` buyouts, `shift.rs`)

The host's owner is the **landlord**: each tenancy pays floor rent to it (`Flow::FloorRent`; a city-owned host's
rent to the Treasury), set at the rent pass like a Home's; unpaid floors evict to vacant. **Founding takes a unit:**
`founding::found_plan` picks a vacant unit within walking reach of the founder's home first, then a Lot (the Lot
cap stops binding); a buyout buys floors, not a block (3b's `[company_housing.buyout]`); a tenancy has its own hours
(`Building.hours`, its kind's table by default, an owner's call later). New keys in `[tenancy]` (rent shares, reach,
eviction days), documented like their neighbours. The coin identity must hold (the 20k run's identity line, then
`core_20k` at the close). Statistical mirror: rent and founding are daily passes on both tiers. **Camera:** a
shop-house with a different sign per unit; a founding's new unit lit. **Shadow:** owner_broke, vendor, madam,
black_doc, forger (20k): a founding into a unit, a rent paid or an eviction. **Numbers:** foundings a week, vacant
units by district, Lots of 8,000 (by now sites of 8,000), employed share d30, home-to-post median, money in rent.

**C6b as built (2026-10-31).** As above (`systems/floors.rs` holds the economy's half, `[tenancy]` its keys), with
these deviations: (1) **The site is picked at the desk:** `found_plan` still walks the founder to the nearest
registry (F1) and `Register` picks the site, as it picked the Lot: a vacant unit within `found_reach_ticks` (20) of
his Home door by the walk (`distance::trip_by`, no El; the leisure tier rule kept), else the nearest Lot as before,
else a refit. Most foundings land in the founder's own block (51 of 60 at 20k into a unit, nearly all "his own
block", floor 0): the HK shop-house. **Corps' `Grow` still builds on Lots** (the tenancy Lots included): the Lot cap
stops binding for people, not for corps. (2) **Vacant units stay non-entities**, materialised by a founding (a new
tenancy `Building`, one storey, `founding::convert`'s seats, staff and venue); a unit records no kind (shop, office,
works floor), so any `unit_kinds` business takes any unit, on the storey `floors::unit_floor` picks (under a Home's
flats the least taken shop storey, else the lowest storey no use stands on). A floor taken back from a `unit_kinds`
business stays an entity: a **tenancy Lot** (counted as used; founding's Lot path and `Grow` build on it).
`World.floor_index` (host -> tenancies, the vacant units, the units founded this run) is not saved, rebuilt at the
seed, the load, every founding, lease or vacated floor and each rent pass. (3) **Who pays and who is put out:** a
corp or the city pays in full (`charge`, as upkeep: its bankruptcy path is the consequence), a person or a gang
what it holds (`pay`); only a person's floor is taken back, after `evict_days` (7) short in a row. A gang's
Hideout is never put out (nobody evicts a gang; its arrears stand: most gang purses pay 0). A kind no unit takes (a
Home, a Fixer, a Mission, a Hotel, a FightPit) is not emptied: its lease passes to the landlord with whoever lives or
works there (`Floor` "took back the lease"), so no household is put out and no new harm route opens; a `unit_kinds`
business goes back to a vacant unit (its staff let go, the customers put out at the door, `Floor` "put ... out").
(4) **The rent:** `floor_rent[tier]` (1, 2, 4: the Home rent's base) a storey of its span, x a corp landlord's
Housing level, plus `revenue_share` (0.05) of its last seven days' mean revenue (a percentage rent), set and paid at
the midnight pass (`rent_pass`, before the upkeep), not accrued as a Home's `rent_due`. (5) **A buyout leases
floors** (`company::lease_floors`): with `[tenancy]` on, the block for sale with two storeys or more gives its top
storeys (the want's bunks, `floors_max` at most, the host keeping one) as a Home tenancy of company bunks, the lease
paid once to the landlord (`block_value` of the storeys x `price_mult` x `lease_mult` 0.5, `Flow::Sale`), the host's
span shrunk; nobody is given notice (the leased storeys were the Ambient tier's flats). A host with no span (the 2k
city) is bought whole as before. **No buyout fired in the 20k run, before or after** (company_buyouts 0 in both);
forced once on a scratch config (`period_days` 1, `min_want` 1, 20k x 3 days) 19 leases stood ("Militech 5 leased
floor 33 of Block#1532 from Stackwell 3 for 240: company housing for Construction Yard#6471's staff (30 bunks)"), the
far staff moved in ("Volt Sallow moved into Block#30321, Greenline 8's company housing for Vat Farm#5679's staff")
and the identity held. (6) **Hours:** `Building.hours` (`None`: the kind's
`[tenancy] hours` row, `default_hours` 08:00-22:00); nothing in the sim's behaviour reads them yet (an owner's call
later): the view's signs do (`ViewUnit.lit`). (7) **Statistical mirror:** the rent pass is building-level, blind to
tier; a founder is a body (`lod` promotes an agent who can found, `founding::can_found`, now O(1) on the index), so a
founding into a unit is as a founding on a Lot on either tier. (8) **Camera:** cityview draws a sign per unit (a
3.2 x 1.5 m slab on the host's facade by its door, on its storey, units on one door spread along the wall; the
colour its kind's, lit while open, dim when shut, white the day it was founded; a placeholder keeps its spawn
material, so each state is its own entity), and `--follow` takes a building (the camera at its door). (9)
`SAVE_VERSION` 120; `EventKind::Floor`, `Flow::FloorRent` (taxed as rent), the `Jobs2Cols` `flow_floor_rent`,
`floor_short`, `floor_evicted`, `floor_taken`, `unit_foundings`, `floor_leases`, `units_vacant` columns and
`run --report`'s `#floors` lines (vacant units by district). No test added.

**The numbers** (seed 42 x 30, `--scaled --ambient-flats 50`, 8 threads, one run each; before is main `a8c08c9`, the
100k before C6a's after; the 20k runs shared the box with builds, the 100k after ran mostly alone):

| | 20k before | 20k after | 100k before (C6a's after) | 100k after |
|---|---|---|---|---|
| foundings (a week) / into a unit | 62 (14.5) / 0 | 60 (14.0) / 51 | (not printed) | 336 (78.4) / 247 |
| vacant units, the city (day 1 -> day 30) | 18,441 (day 1) | 18,437 -> 18,293 | (not printed) | 9,875 -> 9,691 |
| vacant units d30, D1 / D2 / D3 / D4 / D6 | 6,829 / 4,106 / 2,462 / 4,562 / 482 (day 1) | 6,826 / 4,106 / 2,345 / 4,534 / 482 | (not printed) | 3,966 / 2,997 / 1,228 / 994 / 495 |
| vacant Lots d30 (of the generator's 1,600 / 8,000) | (not printed) | 930 | (not printed) | 4,924 |
| employed d30 (share) | 11,839 (0.525) | 11,896 (0.527) | 57,458 (0.513) | 56,942 (0.509) |
| homeless d30 | 314 | 284 | 2,249 | 2,359 |
| home-to-post median d30 | 9 min | 9 min | 4 min | 4 min |
| legs over 120 min (30 d) | | | 19 | 28 |
| Home rent / floor rent a day (mean) | 5,732 / 0 | 5,732 / 4,421 | (not printed) / 0 | 6,170 / 28,677 |
| floor rent short (tenancy-days) / to vacant / leases to the landlord / leases of floors | | 305 / 5 / 4 / 0 | | 2,699 / 53 / 19 / 20 |
| coin identity (every day) | 1,258,977 | 1,258,977 | | 6,443,772 |
| ticks/s days 25-30 | 155/148/161/156/162/155 | 172/169/168/164/167/166 | 12/12/11/12/11 | 12/12/12/12/12/11 |
| peak working set | | | 1,966 MB | 1,954 MB |

The 20k city is flat past one run's noise (employed 0.525 -> 0.527, homeless 314 -> 284, not diagnosed); 51 of its
60 foundings took a unit (30 Noodle Bars, 12 Garages, 9 Ripperdocs; 47 in the founder's own block, 47 on floor 0). At
100k 247 of 336 foundings took a unit and the buyouts leased floors 20 times (none fired at 20k). The floor rent is
the new pressure, and a large one: 28.7k a day at 100k against 6.2k of Home rent (mostly the `revenue_share` on the
corps' Markets and venues, paid in full), a print for the calibration pass, not retuned. The 100k run's 134
bankruptcies include 61 at tick 0 (Stackwell, Militech and Zetatech copies with no buildings: the seeding's, not
C6b's). Nothing retuned.

**Camera** (`c6b/cam`, from the shadow's day-22 world saved at 08:10, `--hide-mass`, `CV_NO_GEN=1`): `c6b_5749_d22_2200.png`,
Bar#5749's stack in D3 with its twelve tenancies (a Bar, Dens, Arcades, Noodle Bars) a sign each along the facade, a
hue per kind, lit at 22:00; `c6b_1516_d22_1200.png` a Mid-Levels shop-house its residents founded into (two Garages
on the ground, a Ripperdoc above); `c6b_1368_d22_0800.png` / `_1200.png` Block#1368 before and after Seraphine Park
registered Ripperdoc#13378 in its vacant unit at 08:05 (the new sign burns white). **Shadow** (20k days 20-23,
owner_broke x 2, vendor, madam, black_doc, forger, `target/c6b/shadow`): `owner_broke_9710` (Anselm Fletcher, a
guard who owns a Ripperdoc in his own block) "Owns: Ripperdoc#6162, floor 0 of Block#690 (floor rent 7 a day to
Habitat 7)", "d20 00:00 Floor: Anselm Fletcher came up short on the floor rent of Ripperdoc#6162 (floor 0, Block#690)
to Habitat 7: paid 2 of 7", "d21 00:00 FLOOR RENT paid -6c to corp Habitat 7"; `owner_broke_7453` (Flux Vasquez)
"Owns: Noodle Bar#33574, floor 0 of Block#4083 (floor rent 1 a day to the city, 1 days short)", her flat on floor 6 of
the same block, sold in a fire sale at d20 00:07. The madam's Brothel, the forger's Fixer and the black doc's Back
Room stand on floors (the madam's "floor 0 of Block#1614"); none of the six founded or was put out in the window
(the run's lines: "Nyx Coldwater registered Noodle Bar#33847 in a vacant unit on floor 0 of Block#5305 (his own block, 2 more vacant there) for 120" on day 16, ten days later "the city put Nyx Coldwater out of Noodle Bar#33847 (floor 0,
Block#5305; the floor rent unpaid): the unit stands vacant"). What reads wrong: a gang's Hideout pays nothing (most
gang purses are empty: "The Drowned Choir 10 came up short ... paid 0 of 2") and nothing follows; a Fixer whose
lease passes stands as a housing corp's ("Habitat 7 took back the lease of Fixer#6185"); a Garage's percentage rent
reads its gross fares (32 a day); the diary's `[money]` line names the day's largest flow beside the net ("+9c
(FloorRent)" on a day the rent went out).

### C7, levels, portals and pathing (TheCity; `components.rs` `Position`, `exec/bigpath.rs`, `exec/mod.rs`, `motion.rs`, `map.rs`, `import_graphics.rs`, `[levels]`)

**A z on positions:** `Position.floor: u8` (0 the street), on the street too, for a body on a deck. **Cores:** each
host carries its stairs and lifts (count, floors served) from the export; a climb is a timed leg inside the
structure, `levels::climb_ticks(host, from, to)`: by stairs `[levels] stair_ticks_per_floor`, by lift `lift_wait_ticks`
plus `lift_ticks_per_floor`, a lift taken past `lift_min_floors`; the numbers are set at the game's pace (on screen a
stair flight is ~2 tiles walked, so the 40th floor by the stairs is most of an hour and the lift is worth waiting
for). Full: a climb step at the door between the street path and the `Use` (an `ExecState` arm); Coarse: the climb
added to `timed_walk` and placed by `walk_place` at the door with the floor interpolated. **Portals outside
buildings:** the map grows `P <kind> <x,y,level> <x,y,level> <ticks> [hours=<h0>-<h1>]` lines from
`walkways_sim.json` (footbridges, the escalator one-way by the hour, ramps, station stairs); `bigpath::find` adds a
tile's portals as neighbours (the escalator's direction read at the search's tick), `labels` merges components
through them, and the landmarks are measured over tiles plus portals (`dial` with the jump edges; a portal that
shortcuts what the landmarks measured would make the heuristic inadmissible). Commute estimates stay Manhattan
(the D2 footbridges span one street each; they shorten little and the error is the estimate's today). The path
carries the level per step; a save carries it (`SAVE_VERSION`). Statistical mirror: none (a Statistical agent does
not walk). **Camera:** the subject climbing to its floor, a lift wait at the lobby, a crowd on the D2 footbridges at
the evening rush, the escalator running up after 10:00. **Shadow:** exec (Spire tower), office_worker (D2),
super or sweeper on a high floor, courier (20k). **Numbers:** climb ticks a day (mean, top-floor households), legs
over 120 min, footbridge and escalator crossings a day, `bigpath::expanded` per walk, landmark build, ticks/s at
100k days 25-30 against C6b's run (the accepted floor is ~25, `OPEN-226`).

**C7 as built (2026-11-01).** As above (`systems/levels.rs` holds the climb and the lift, `exec/bigpath.rs` the
portals' graph, `[levels]` its keys), with these deviations: (1) **The pace.** At the real pace (Y4: 21 tiles a
minute) "a flight is ~2 tiles walked" is 6 s, so the stairs are set as a person climbs, not as tiles:
`stair_ticks_per_floor` 0.5 (30 s a storey up, an ordinary adult's sustained climb; down at `stair_down_mult` 0.5),
**the 40th floor is 20 min by the stairs, not "most of an hour"** (that figure was the pre-Y4 arithmetic of two tiles
a flight at a tile a minute); the lift `lift_wait_ticks` 1 + `lift_ticks_per_floor` 0.05 (the 40th floor 3 min with
the wait), taken past `lift_min_floors` 3 when its wait and ride beat the stairs. Ticks are fractions, a climb rounds
up. (2) **The lift as a place is a booked fluid queue, not an entity per car:** `World.lifts` (saved) holds each lift
core's calls by the tick; the queue at a tick is `max_s (calls in (s, t] − rate × (t − s))` at the cars' rate
(`lift_capacity` 13 a car a cycle, the cycle up to the lift's top and back plus two `lift_dwell_ticks`), and a call
waits `lift_wait_ticks` plus a cycle (over the cars) for every full car ahead; a crowd that makes the lift slower than
the stairs takes the stairs. A Full call is booked at the door, a Coarse leg's at its first step for the tick it will
reach the lobby. **C8's hooks:** `LiftState.cut` (no car runs: every climb takes the stairs; `levels::plan` reads
it) and `held_by` (read by nothing yet). The bodies in the lobby are the Full bodies in their `ExecState::Climb`
before `board` (at the door, on the ground). (3) **The cores:** a host carries `lift=<cars>:<top>` (its member lots'
lift cores: their count and the highest storey; 4,295 hosts), a tenancy climbs its host's; every structure has its
stairs (the export's rule). (4) **The Full climb** is an `ExecState::Climb` arm: up after `try_enter` lets the body in
(`climb_in`: in the lobby on the ground until it ends, then the GoTo is done), down before the walk out of a building
(`climb_out` in `advance_goto`, the walk's path kept and walked on after it); a move between two buildings of one
structure (a span on either) is one climb between the storeys, through no door. **The Coarse climb** is
`GotoTimed.climb` (`TimedClimb`: down, then the walk, then up; `exec::walk_route` returns the walk between them),
placed by `walk_place` / `climb_place` at the door with the storey interpolated. Rides, cabs and flyers do not climb
(their door legs are not walks): a finding for C9. (5) **The `P` line** is `P <kind> <x>,<y>,<level> <x>,<y>,<level>
<tiles> [hours=6-10ba,10-24ab]`: the run in **tiles**, not ticks (at the real pace a 15 m walkway is 0.2 min); the sim
adds the climb from `[levels]` (`portal_stair_ticks_per_floor` a storey of the ends' levels for stairs, exits and
landings; a station lift `portal_lift_wait_ticks` + its ride; the escalator its run at `escalator_speed` 1.5). A lot end
(a landing) is the street outside the lot's nearest door at level 0, the walk to it added to the run; a deck segment
past 4 tiles is cut into pieces (249 portals as 529 lines, 403 deck nodes) so a walker's hop is a few tiles like any
step. (6) **The search:** deck nodes are numbered after the tiles; an edge costs its steps at the Road rate (at least
7 × the ends' Manhattan) plus the entry of the tile it reaches, the tile steps' shape, so the landmarks' second bound
still holds; the escalator is the exception (a moving stair is quicker than the street, and on the flat ground its
lift up the hill is no advantage, so at 7 × Manhattan it never paid: the escalator test caught it), and the Manhattan
bound is less the most every portal under it saves together (`Portals::saving`, ~200 tenths): admissible, not
consistent across the escalator, where a node may be reopened. Drivers search without the portals. A walker's hop is
checked at the hop (the escalator's direction at that minute; a stale hop re-finds), `Position.floor` is the deck's
level in the street. (7) **Commute estimates stay Manhattan** (`timed_walk` untouched; the climbs are added in
`timed_goto` only). (8) `SAVE_VERSION` 121; both maps re-imported (`lift=` and the `P` lines; the `B` order is
unchanged); the view fixture regenerated (185 KB). **Statistical mirror:** none: a Statistical agent does not walk or
climb, and promoted he stands on his floor (`floors::floor_in`). **One test** (`test-add`): on the 20k map the walk
from the escalator's foot to its head rides it at noon and not at 07:00 (it runs down), and the search's cost over
the portals equals the Manhattan-only search's.

**The numbers** (seed 42 x 30, `--scaled --ambient-flats 50`, 8 threads, `--legs`, one run each; the before is this
branch's binary with `[levels]` out of the config and the pre-C7 map, so the search is the old one with no portals; the 100k after shared days 8-13 with the view tier's build, days 25-30 ran alone):

| | 20k before | 20k after | 100k before (C6b's after) | 100k after |
|---|---|---|---|---|
| climbs a day (mean min) | 0 | 11,517 (1.8) | 0 | 59,659 (2.0) |
| climbs to floor 20+ a day (mean min) | 0 | 2,184 (3.0) | 0 | 14,914 (3.0) |
| lift rides a day / mean wait / worst lobby queue | 0 | 6,265 / 1.0 min / 7 | 0 | 40,768 / 1.0 min / 45 |
| footbridge / escalator / stairs-lift-platform hops a day (Full walkers) | 0 | 4 / 838 / 52 | 0 | 22 / 3,570 / 234 |
| deck entries a day | 0 | 27 | 0 | 122 |
| legs over 120 min (30 d) | 2 | 8 | 28 | 32 |
| `bigpath` searches / expanded per search | 661,703 / 567 | 682,999 / 910 | (not printed) | 3,526,971 / 706 |
| searches that found no path (labels connected through a portal) | 0 | 19 | | 142 |
| landmark build | 138 ms | 149 ms | (not printed) | 213 ms |
| employed d30 / homeless d30 / home-to-post median | 11,896 / 284 / 9 min | 11,747 / 327 / 9 min | 56,942 / 2,359 / 4 min | 56,813 / 2,439 / 3 min |
| ticks/s days 25-30 | 147/144/145/138/144/143 | 130/133/132/125/127/125 | 12/12/12/12/12/11 | 12/12/11/11/12/11 |
| peak working set | | | 1,954 MB | 1,998 MB |

The climb is cheap to the sim and visible in a life: 11.5k climbs a day at 20k (1.8 min mean; to the 20th floor or
higher 3.0 min), 6.3k of them by a lift with no queue (the worst lobby 7 calls, under a car); the lift is the norm in
the Mid-Levels towers and the stairs in the Sump's walk-ups. At 100k the lobbies queue (45 calls at the worst),
still mostly within a car. **The walkways barely carry anyone:** the escalator 840 hops a day at 20k (a ride is ~50
hops: ~15 riders a day, up after 10:00, down 06:00-10:00), the footbridges 4: on the flat ground with no traffic at
grade a footbridge is never shorter than the crossing under it, so only a podium's cut through a block pays (the
spec's "they shorten little"); no crowd on the D2 bridges forms at the evening rush (the camera found the decks empty).
The search costs more: `bigpath` expanded per walk 567 -> 910 at 20k (finds +3 %), of which a third is the portals in
the landmarks and the rest the climbs' shift of who walks when (measured apart on day 1: 583 -> 678 with the portals only,
-> 871 with the climbs); ticks/s 20k -11 % (144 -> 128 days 25-30: at the ~10 % line, a finding for a perf phase, not a knob). At 100k the run is C6b's within noise: 12 -> ~11.5 ticks/s days 25-30 (~4 %, under the ~25 floor already, `OPEN-226`), 1,998 MB at the peak, legs over 120 min 28 -> 32 (the open leg records of 2-3 days the leg log keeps, none a climb), expanded per search 706. Nothing retuned.

**Camera** (`target/c7/cam`, from the shadow's day-22 save, `cityview --save --follow --bench one --shots`):
`c7_climb_8408_d22_0619.png` / `_0620` / `_0621`, `office_worker_8408` (Slate Singh) home at 06:18: in Block#1439's
lobby at 06:19 (the camera at z 49 m, the ground), in the lift at 06:20 (z 83) and on floor 22 at 06:21;
`c7_escalator_22263_d22_1008.png` / `_1010.png`, Ottilie Volkov riding the escalator up at 10:08-10:12 (the hop log of
the follow-cam's own world: 52 hops from (505,704) to (505,506)), drawn on its deck over the road (z 63.9 m);
`c7_d2bridges_5759_d22_1745.png`, Bar#5759 under the D2 footbridges at the evening rush: nobody on the decks (above).
**Shadow** (20k days 21-22, exec x 4, office_worker x 3, super x 2, sweeper x 2, courier x 2, `target/c7/shadow`;
the diary's `[climb]` and `[deck]` lines are new): `office_worker_8408` "d21 08:09 CLIMB down by the lift in Block#1439
(D4 Mid-Levels): floor 22 to 0, 1 min waiting in the lobby, 3 min in all" and back "d21 18:25 CLIMB up by the lift ...
floor 0 to 22"; `courier_12172` in a D1 walk-up "d21 00:01 CLIMB up by the stairs in Block#2518 (D1 The Sump): floor
0 to 8, 4 min in all", down 2 min at 06:37; `sweeper_20564` "d21 02:16 CLIMB up by the lift in Block#1009 (D4
Mid-Levels): floor 0 to 19, 1 min waiting in the lobby (2 in the queue), 2 min in all"; `super_16201` his own floor 7
by the lift, his round's buildings by the stairs to floor 1. What reads wrong: **the Spire's execs live on floors 0-3**
(D5's estimates are four storeys: no exec climbs a Spire tower; `exec_25360` "CLIMB down by the stairs in Block#341 (D5
The Spire): floor 1 to 0, 1 min"); a visitor and a Super on his round climb to the span's lowest floor (C6a's rule); no
pick crossed a footbridge or the escalator in the window (no `[deck]` line; the escalator's riders are found in the hop
log); the Corp HQ and other whole cuts have no floors, so an office worker climbs at home and not at work.

### C8, the vertical gangs (TheCity; `turf.rs`, `raid.rs`, `crew.rs`, `gang.rs`, `plan.rs` `bed_of`, `founding.rs`; `OPEN-191`, `OPEN-241`, `OPEN-187`)

**A Hideout per district:** a gang holds a Hideout tenancy (upper floors, often over a front it runs: a restaurant,
a Bar) in each district where it holds turf, founded or bought into a unit by C6b's rules; `bed_of`, the muster
(`raid::pick_muster`, `muster_candidates`), the raid's departure and the lie-low take the nearest of the gang's
Hideouts, so `OPEN-191`'s Sump bed and `OPEN-241`'s members 5-7 h from the muster go at their cause. **Vertical
turf:** a Home claim is on a tenancy (a span), so one tower holds two gangs' floors and a buyout or a claim moves
floor by floor; the turf cell's holder is still presence on the street. **Raids floor by floor:** the column climbs
(the lift lobby the choke: a raid takes the stairs past `lift_max_raiders`), passes the floors between (their
tenants hear it: the noise is word), fights at the target's floor; the law's response climbs too. Under 18 nobody
is a target on any floor (the hard rule; `harm_blocked` printed). Statistical mirror: the order-rates ledger reads
the Hideout's district as it reads the old one. **Camera:** a raid climbing a tower's stairs, the Hideout's lit floor
over the front's sign. **Shadow:** gang_leader, gang_member, raider, pimp, guard (20k), then the same five in the
100k run: a raid that lands, quoted from a member's diary. **Numbers:** Hideouts per gang, Hideout legs 4 h+ (F2's
119), musters reached, raids landed, wars, Succession lines, `harm_blocked`.

### C9, far travel re-read on the new city (TheCity; `distance.rs`, `careers.rs`, `household.rs`, `social.rs`, `relocation`; `OPEN-221`, `OPEN-240`)

First a read, then the fix for what is left. One 100k run in V17's window with the travel archetypes' diaries
(exec, guard, cabbie, cook, builder, loader, night_shift, worker_friday, companion, gang_leader, gang_member,
day_labourer) and the legs over 120 min listed by cause (the post, the move-in, a goal that ignores distance, the
patrol, the muster). Then fix the sim half that remains at its cause: the move-in's distance check (`OPEN-240`'s
marriage move-in 8 h from the post), "moved nearer work" choosing by travel not tiles, leisure and errands within a
walk, the patrol's start. The pace itself was fixed in round 9 Y4 (Dylan's answer 1). **Camera:** the follow-cam on the
farthest commuter left. **Numbers:** legs over 120 min, workers past 3 h, home-to-post median, the top ten causes,
20k before and after plus the read's 100k run.

## The exit

One read at 100k seed 42 (Shadow V19, the city) and one watch: (1) a diary whose home, post and evening fall within
a walk in or beside one tower, and the 100k legs over 120 min well below C5's; (2) a gang holding floors over a
front, and a raid that climbs and lands in a member's diary; (3) no Hideout leg past 4 h; (4) the follow-cam, on
three V19 subjects, showing a climb to a floor, a footbridge or the escalator crossing, a ride on a routed El line
and one tram, with the PNGs in the report; (5) no minor harmed, `harm_blocked` printed; (6) ticks/s at 100k days
25-30 at or above ~25, the change tier within 4 min with no Bevy in it, the view tier green and timed. What reads
wrong goes to OPEN; the view milestone (`OPEN-149`, `OPEN-180`'s rest) follows, then M18.

## Risks and costs

- **Build and disk:** a cold Bevy build is minutes and ~5 GB of `target/` in each worktree that builds the view
  (TheCityGraphics' `lighttest/target` is 4.7 GB); only C1, C2 and the close build it, and the rule to delete a
  merged worktree's `target/` covers it (359 GB free on D: today).
- **The change tier's 4 minutes:** safe once C1's flag table lands; the risk is a missed `--workspace`, a lock bump
  under the sim (C1 checks `cargo tree -p citysim`), or a view test counted by the lint (it counts `citysim/` and
  `citysim-cli/` only). **The 2k city has no floors or portals,** so the change tier never runs C6-C8's code; giving
  `mapgen` tenancies would re-pin a share of the 1,470 tests on the 2k city. Accept it: the 20k runs and `core_20k`
  at the close are where the floors are exercised, and a phase reads its own coin identity line.
- **Perf at 100k with a z:** the Building store grows ~2x (tenancies only), `with::<Building>()` passes with it;
  portals add neighbours to a few hundred tiles and landmarks are measured once a map; the climb is O(1). C6a and C7
  print ticks/s and the working set; a drop past ~25 is a finding for a perf phase, not a knob.
- **Cross-repo:** TheCityGraphics' `out/` is gitignored and regenerated by hand, so the import names the sha and the
  export seed; C3 and C4 share one disk's `out/` and stay serial. A rail route that needs a kit regen is a 4.1 h job
  left running, outside a brief.
- **Saves and fixtures:** each of C5, C6a and C7 bumps `SAVE_VERSION` and changes the `B` order; the view fixture is
  regenerated each time (`export-view`).
- **C6a/C6b size:** tenancies touch the importer, the store and every one-building-per-cut assumption; X1 and X2
  (new systems end to end) ran 444 and 389 turns. The briefs name the anchors and hand back at ~300.

## Dylan's answers (2026-10-30)

1. **The pace is fixed now, not kept**: pressure pass round 9 Y4 makes a walk real walking speed (several tiles a
   tick, ~21 at 5 km/h on 4 m tiles) before Shadow V18, so every phase here is built at the real pace. The
   walk-matched view is then about real time (an RPG's 1x); faster speeds are for watching. Dylan 2026-10-31: the
   pace will be tuned visually once the follow-cam stands (C2), and the cells' load (`OPEN-256`) waits for that. C7's climb numbers and
   C9's read are at that pace.
2. **`k = 2`** with tenancies.
3. **The landlord**: a tower's owner rents floors (a floor rent flow, evictions by the floor); buyouts by the
   building or the lease.
4. **The camera ring** (not asked; the recommendation stands): C2 pins the ring as a stopgap, `OPEN-149` stays the
   view milestone's.
5. **Lifts are a place too**: a lift car and its lobby hold bodies (a wait, a ride you could watch), can be held by a
   gang, cut (a landlord who stops paying the power, a sabotage) and fought in. C7 builds the lift as a place with
   capacity and a queue; C8 adds holding and cutting it as turf and raid moves.

6. **Wave 1-2's calls (Dylan 2026-10-31, "Sounds good"):** C3's rack lines (Mid-Levels and Hillside at up to 18 % /
   16 %, platforms on the grade), Hillside stacked over Mid-Levels for ~70 m on link_4, and Mid-Levels ending at Wing
   Lun Road are kept; C4's D2 office density (one per 500 m2 of plate, 4,030 offices) is kept as a placeholder until a
   D2 layout pass. The view's base speed is real time (walk-matched, 1/60 ticks/s: a game minute is a real minute,
   an RPG's 1x; faster speeds are for watching). The view tier's budget is 7 min (a cold run rebuilds the sim).

7. **C5-C6a's calls (Dylan 2026-10-31, "Seems good"):** D2's offices hold work (1,223 Lots at 100k; commutes into
   the Civic Core) and `farm_desk` keeps Farms off office stacks, both kept. Bodied riders barely use the El at 20k
   (~1,780 of ~1,785 boardings Statistical, at the real pace a few blocks walk faster): leave the ride choice alone;
   C9 reads it at 100k, and if no body rides cross-island there it is a feature (the El as the way to a far job,
   `OPEN-254`), not a knob.

## The questions as asked


1. **The pace.** A walk is 4 m a game minute (1/21 of a human's), so far travel is pace as much as map; and R2's
   1-tick tiles made the walk-matched view 0.35 ticks/s (a ~69-min game day), not the 0.7 you settled on
   2026-10-23 (`OPEN-180`). Keep the pace and localise life (tenancies, the El), or walk several tiles a tick (bodies
   cross the island in game hours, but at any watchable rate they run on screen)? **Recommend: keep the pace,
   localise life, X1 at 0.35 ticks/s with faster speeds for watching;** C6-C9 are built on that.
2. **Footprint: `k = 2` with tenancies, or `k = 1`?** `k = 1` doubles the cuts (8,765) and halves every walk's pace
   (2 m tiles at one a tick). **Recommend `k = 2`;** the 20k units on today's cuts are the sites 100k needs.
3. **The landlord.** Should a tower's owner rent floors to businesses and gangs (a new rent flow, a landlord class,
   evictions by the floor), or should a tenancy's owner own its floors outright? **Recommend the landlord:** it is
   the pressure (rent) the city runs on, and a gang over a front pays someone.
4. **The camera ring.** C2's follow-cam pins everyone in the ring (a stopgap); the real ring (`OPEN-149`: its own
   Full budget, promotion before visibility, Statistical placed by schedule) is the view milestone's. Pull it into
   this milestone? **Recommend not:** the follow-cam is a watching tool here; "nothing in camera range is
   Statistical" binds when the player arrives.
5. **Lifts as a mechanism.** Lifts as travel cost only, or also a place that can be held, broken or cut (a gang's
   lobby, a landlord who stops paying the power)? **Recommend cost only in C7,** the lobby as the raid's choke in C8,
   breakdowns later.
