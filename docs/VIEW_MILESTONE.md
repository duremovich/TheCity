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

### V7, crossing the street and the pace by eye (`OPEN-260`, the walking pace)

With the 3D world to look at: ground vehicles that stop, lights or marked crossings, and a crossing cost so the D2
footbridges carry their share. The walking pace and the view rate tuned by eye through V1. Design calls for Dylan at
the phase's planning.

### V8, leftovers the view exposes (`OPEN-264`, `OPEN-256`)

The raid's climb is the member's body (`OPEN-264`); the cells' week (`OPEN-256`: bail, fines, overcrowding, a
varied cell day). Split or moved at planning by what V1-V7 leave.

## The exit

1. A resident followed live at 100k for a game day at real time and at x60 with no sim stall (no tick over ~2 s).
2. Everyone in the ring drawn as a person, animated; riders inside their car; the camera on the body's floor.
3. Nobody in the ring Statistical; promotion places bodies from their schedule.
4. A recorded shadow pick's day plays in cityview and agrees with its diary.
5. A live key reloaded changes the view within a tick; the pace set by eye.
6. 100k ticks/s at V18's or better.

Read at the close as Shadow V20 plus a watch (the camera's shots per exit line).
