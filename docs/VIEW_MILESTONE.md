# The view milestone: watch the city live, people drawn as people, the ring Full

Drafted 2026-11-01 against TheCity `0f54230` after the city milestone's close (Shadow V19). The roadmap's line
(`docs/ROADMAP_POST_M14.md` addendum 22): the Bevy client reads the sim and follows an NPC, no player input; M18 is
rewritten against this client afterwards. The doctrine holds throughout (CLAUDE.md): no test unless a phase names
one, a phase is proven by its shadow read and the follow-cam's shots, every number in `assets/config.toml`, no child
is ever harmed.

## Why

- **The client draws the city and the sim runs in it** (C2's follow-cam: `cityview --save --follow --shots`). The
  first look (2026-11-01, `office_worker_42443` on the 100k day-21 save, Dylan: "it is a city at least") showed what is
  missing: residents drawn as cream capsules, some of them floating on facades; riders on a white slab (`OPEN-271`);
  the sim stalls 14.5 s at 21:00 every day (`OPEN-265`); a replay parts from the diary within hours (`OPEN-270`).
- **Tuning by eye needs a fast loop**: Dylan chose config hot-reload over a scripting layer (`OPEN-262`); the pace
  (`OPEN-256`'s walking pace, `OPEN-260`'s crossings) gets tuned watching the view, not reading a CSV.
- **Nothing in camera range is Statistical** (standing decision, `OPEN-149`): the follow-cam pins the ring as a
  stopgap; the ring's own budget and promotion from the schedule are this milestone's.

## What exists (the anchors)

- `view/cityview/src/simthread.rs`: a thread owns a `World` loaded from a save, ticks it at the walk-matched rate
  (`[view] walk_m_s`, 1/60: a game minute is a real minute), publishes the ring's `ViewFrame` into a two-slot buffer;
  `--from`, `--shots` (with `--bench one`), `--rate`. `sim.rs` draws a `Capsule3d` per body (line ~482).
- `citysim::view` (`snapshot`, `pin_ring`, `walk_matched_rate`); `[view]` in config (read by the view only).
- A save carries its `Config` (`save.rs`); the 100k world's config is fitted and scaled at build (`[scale] fitted`).
- TheCityGraphics: NPC character rounds (`assets/characters/base_body/npcs/round_01..09`, `street/`), the Mixamo rig
  and clips (`mixamo/anims.json`); glclient's `--squad N` already draws trailing NPCs on the kit.
- The 100k day-21 save of V19: `target/v19/isle100k_s42_d21/saves/42-30240.ron` (583 MB, loads in ~9 s).

## Phases

Sized to one Opus agent each (~300 turns); each lands on the change tier (and the view tier where cityview
changed), its shadow read and its camera shots.

### V1, config hot-reload (`OPEN-262`; `citysim/src/config.rs`, `view/cityview/src/simthread.rs`, `main.rs`)

cityview reloads a config file (`--config PATH`, default `assets/config.toml`) on a key and on a file change (an
mtime poll on the sim thread between ticks; no new dependency unless the coder finds one already in the lock) and
swaps it into `World.config` at a tick boundary. The reloaded file goes through the same fit and scaling the save's
world went through, so a reload of an unchanged file is a no-op. Each section is classed live or build-time (the
map, populations, landmarks, names, anything a world build or a cache derives once); the class lives in one place in
`config.rs`. A reload diffs old against new by key and logs and shows in the HUD: the live keys changed (old ->
new), the build-time keys changed and ignored. Config-derived caches the World holds are rebuilt on a live change
or their section is build-time. The walk-matched rate follows a pace change. Out of scope: a console to set one key
(later, if the loop wants it). **Camera:** a bench flag `--reload-at TICK` reloads at that tick, so a shot pair
before and after a doubled walking pace shows the subject covering twice the tiles a tick, and the log shows a
build-time key refused.

### V2, the 21:00 tick and 100k ticks/s (`OPEN-265`)

Profile the 21:00 tick first (`target/v19/run.csv` `max_tick_ms`, `max_tick_at` 1260: 14.5 s, V18 2.3 s); the
tenancies' growth (C6a/C6b) landed in a nightly system. Then the steady ticks/s (11 on days 24-29, V18 13-14). A
phase that claims a number gives before and after, one 100k run each. **Exit:** no tick over ~2 s, ticks/s back to
V18's. The view at real time never stalls on the sim.

### V3, people drawn as people (`OPEN-271` drawing; cityview `sim.rs`, glclient's NPCs; TheCityGraphics on its own branch if assets need export)

Replace the capsule with TheCityGraphics' NPC characters: a body's look keyed to its identity (sex, age band,
class, a stable pick from the rounds by entity id), the walk and idle clips from the Mixamo set, a cheap LOD
(skinned near the subject, a static pose or impostor past a distance) for ~2,000 bodies in the ring. Bodies indoors
are not drawn on facades (check what the floating capsules are: indoor bodies at their floor's z, or a z bug).
**Camera:** the 08:00 street and an evening venue, before and after.
Dylan 2026-11-01: agents use TheCityGraphics' branches as needed and bring the parts the client needs into this repo
(`view/`) where they fit; the bulk assets stay behind `CITYVIEW_ASSETS`.

### V4, riders and climbs (`OPEN-271` the rest)

El riders inside a train car, lift riders in a car in a shaft (or hidden with the car drawn), the HUD says the
climb and the floor, the camera follows the body's z up (it falls 79 -> 52 today). **Camera:** C7's lift ride and an
El ride.

### V4b, the orbit camera and the cutaway (Dylan 2026-11-01; `view/cityview/src/cam.rs`, `occ.rs`, `view/glclient/src/occlusion.rs`)

Dylan, after V3's shots: the portal is "not the ideal experience to play the game. Too much building", and locked iso
was chosen hoping it made graphics simpler, which it no longer does. His calls: **an orbit follow-cam** (perspective
at a narrow FOV ~30 deg, free yaw around the subject, pitch clamped ~35-65 deg, zoom by distance; replaces the
orthographic NW view of SPEC §CAM 5.1) and **a cutaway** in place of the 6 m capsule cut: every building on the
camera's side of the subject within a wide box is sliced down to the subject's floor (soft dither at the edges), the
subject's own building sliced at its ceiling so the room shows, buildings behind and beside at full height. The camera
follows the body's z (V4's item 4 moved here) and the cut opens inside a kit building (V4's item 6). **Camera:** the
07:00 Mahjong queue, a tower dweller at home on a high floor, an El platform, each at two yaws.

**As built (2026-11-01).** The camera: `cam.rs`, a perspective orbit (FOV 30, pitch 35-65, the zoom still the frame's
width at the subject so `--zoom` keeps its meaning; `--yaw` / `--pitch`), the aim eased toward the subject with its z
(`[view] cam_*`); the orthographic game camera is gone (Tab's overview stays). The cutaway: one world-space clip in
`occlusion.wgsl` from one uniform (`[view] cut_*`). Deviations: the box has no depth key, it runs from `cut_behind_m`
behind the feet to past the camera (a depth short of the camera left towers between them standing at wide zooms), and
it switches off past `cut_max_zoom_m` (250 m: the establishing shot keeps the city whole). The subject indoors, the
sim's rect of its building is cut at its ceiling (`cut_ceiling_m`), so a room shows in a massing box as well as a kit;
a kit or glb building holding the feet is also cut to its floor through its own cutaway (stubs, room lights). The cut
plane: kits show their slabs; massing is drawn double-sided and its inside painted as the floor plate under the
subject's floor and bare walls above (the ground under a ground-floor room too). Street props and the sim's signs
carry `CutProp`. P4's capsule, its ray casts, hystereses, keep list and L3 deck fades are retired with `occbench`,
`shadowbench` and `bench_occ.sh` (TheCityGraphics `test_occlusion.py` no longer has a bench to run; DECISIONS V4b).

### V5, the ring (`OPEN-149`)

The ring's own LOD class and budget outside `max_full`: everyone in camera range Full, Coarse in a band around it,
Statistical beyond; ranking by camera distance before story class; re-assignment as the camera moves (each tick, not
hourly); Statistical -> Coarse places the body where its schedule says it is now (not the stale demotion snapshot);
the fast mover's ring growth (611 -> 2,107 bodies in eight minutes) bounded. **Proof:** a follow across the island
at 100k: the ring's counts per tier in the HUD, no Stat body on screen, ticks/s printed.

### V6, the camera's replay (`OPEN-270`)

The follow-cam as a proof tool: `shadow` records the picks' ring frames (the `city-simview` file form `export-view`
already writes), and cityview plays a recording (`--sim FILE` reads the file form today). A diary line and the shot
at its minute agree for the whole window. Determinism stays dropped.

Landed (V6): `shadow --record [PICKS]` writes `<out>/rec/<arch>_<index>_d<day>.simrec.gz` (`city-simrec/0.1`: a
header line, then a frame a tick as a delta on the last, gzip'd JSON lines; `citysim::view` `RecEncoder` /
`RecDecoder`, `citysim-cli/src/record.rs`), every pick's ring from one pass (`view::snapshot_many`), `[view]
record_half_tiles` about the pick; the shadow sets no view, so the ring is the pick's surroundings at the tier the
shadow's world gives them (the pick Full, most bystanders Statistical) and recording changes nothing in the diary's
world. cityview `--sim REC` plays it through the follow-cam's buffer (`view/cityview/src/recplay.rs`: `--follow`,
`--rate`, `--from`, `--bench one --shots`). The diary rule (`citysim::view`'s module doc): `--shots T` shows frame
`T + 1`, the state the diary stamps `T`, live or recorded; the caption leads with that minute. 20k: 0.4-1.0 MB a pick
a day.

### V7, crossing the street and the pace by eye (`OPEN-260`, the walking pace)

With the 3D world to look at: ground vehicles that stop, lights or marked crossings, and a crossing cost so the D2
footbridges carry their share. The walking pace and the view rate tuned by eye through V1. **Dylan's calls
(2026-11-01):** lights and marked crossings (a signal cycle per intersection off the tick; a body walks to a crossing
and waits for the walk phase; a footbridge wins when the detour is long). **Traffic is real, no filler:** citizens
driving their own cars, trucks and buses, plus self-driving vehicles, all sim bodies, added later (`OPEN-275`); V7
builds the crossings and signals only.

**V7 as built (2026-11-01).** *Where the crossings come from:* TheCityGraphics draws them, so the import reads them
(`import-graphics`, `read_crossings`): the map's `carriage` grid from the client's ground raster
(`out/chunks/*.ground.bin`: a sim tile all carriageway or median under it, with a road tile on all four sides, so the
outermost road tile is always a walk; 68,765 tiles at k2) and the zebras of `out/street/*.json` as `X` lines (600
after the duplicates; the 423 dropped all cross a `lane`'s 4 m carriageway, which has no carriageway tile at 4 m and is
crossed anywhere; a `mouth`, the side street's dropped kerb, is walk). A re-import of both island maps from
TheCityGraphics `2bcd2c0` is byte-identical but for those two sections and the commit in the comment.
*The cost:* a walker's search (the foot `Portals`, `bigpath::PedCosts`) pays `[crossings] jaywalk_tenths` (150; a Road
step is 7) to step onto a carriageway off a crossing and, on a crossing's tile, the Road step plus the signal's expected
wait ((1 - `walk_share`)^2 x `cycle_s` / 2 = 28 s at 100 s and 0.25) over its depth; never a wall, so nothing becomes
unreachable (the labels are unchanged; no test). The foot landmarks are measured over these costs; drivers search with
their own (`FlowCache::landmarks_drive`, built on a driver's first search). A Full walker reaching a crossing's kerb
waits there for the walk phase read at its sub-tick time (`exec::crossing_wait_q`, `signals::wait_ms`), standing at the
kerb past the tick when the wait runs over it; a timed walk on foot adds `timed_crossings_per_tile` (0.0033, measured
from the Full walkers) crossings a Manhattan tile at the expected wait. *Signals:* `signals.rs`, a pure function of the
time and the junction tile (an offset hashed from it); `vehicle_go` is OPEN-275's hook (the main road moves in
don't-walk). *The view:* the zebra paint was glclient's already; `cityview/src/lights.rs` stands a pole at each kerb of
the crossings within 160 m of the subject, a pedestrian head (red man over green man) and a vehicle head (red, amber,
green), lit from `signals` at the view's game time; it follows a config reload. A save from before V7 is grafted the
checkout's map of the same name and `--config`'s `[crossings]` (`lights::graft`). Not built: a waiting body is drawn
lerped through its tick, not standing at the kerb for the don't-walk.
**Numbers** (20k seed 42 x 7, `run --population 20000 --scaled --ambient-flats 50 --map assets/maps/isle_m_k2.txt
--report --legs F --legs-min 0`, before = main `bb90eb2`'s binary on the pre-V7 map): footbridge riders (steps up onto a
footbridge) ~2 a day before (17 deck entries a day, the escalator's ~15 among them) -> 76 a day (103, 89, 90, 92, 49,
60, 46), footbridge hops 4 -> 470 a day; mean walk leg 7.56 -> 7.75 min, mean walk leg to Work 3.93 -> 4.27 min (Full
4.06 -> 4.27); 11,500 crossings a week by Full walkers, 26 s waited at each; 280 jaywalks a day (the Vats' freight
roads have no zebra near: a courier jaywalks the same carriageway four times a day); ticks/s 128 -> 126 (both runs
shared the box with three other coders: a print).

### V8, leftovers the view exposes (`OPEN-264`, `OPEN-256`)

The raid's climb is the member's body (`OPEN-264`); the cells' week (`OPEN-256`: bail, fines, overcrowding, a
varied cell day). Split or moved at planning by what V1-V7 leave.

**As built (V8):** the raid's column is bodies (`vertical::climb_column`): the first at the door picks the lift or
the stairs, every raider steps in and climbs himself (`ExecState::Climb`, his own pace on the stairs past
`[vertical] stair_age_from`), the Brawl goes on at the top and the fight waits for the last body; a tail marcher
climbs after; the raid keeps the rival it was ordered on (`raid::raid_rival`); a floor Hideout is walked into by its
founders (`found_walkers`); the hurt and the dead print ages. The cells' week is `[cells]` (`systems/cells.rs`):
fines at booking and bail from the purse (`Flow::Fine`), the crowded Precinct's early releases, the work detail and
cards, the BreakOut dropped with a reason. Deviation: no shadow diary shows the climb, as no gang raid reaches a
rival's door on the current maps (`OPEN-276`); the climb was read on the 2k city with a seat lifted to floor 6.

## The exit

1. A resident followed live at 100k for a game day at real time and at x60 with no sim stall (no tick over ~2 s).
2. Everyone in the ring drawn as a person, animated; riders inside their car; the camera on the body's floor.
3. Nobody in the ring Statistical; promotion places bodies from their schedule.
4. A recorded shadow pick's day plays in cityview and agrees with its diary.
5. A live key reloaded changes the view within a tick; the pace set by eye.
6. 100k ticks/s at V18's or better.

Read at the close as Shadow V20 plus a watch (the camera's shots per exit line).
