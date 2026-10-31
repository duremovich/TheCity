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
