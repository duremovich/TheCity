# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-11-01, the thirtieth session: the city milestone (C1-C9a) built and closed, Shadow V19.**

## Main

Main holds **the closed city milestone** (`docs/CITY_MILESTONE.md`, each phase's "as built" and § "Closed
(2026-11-01)"): the Bevy client in the workspace, the follow-cam on a save, the El on rail, floors as tenancies, levels
and lifts, vertical gangs, the Spire's storeys, far travel re-read. Change tier green on main after the close's fix
round (`[change] GREEN; BUDGET OK 3m37s of 4m00s`, on `d41f927`); tests 1,472; `SAVE_VERSION` 125;
`assets/stat_table.toml` is the pre-regen table (the 2k regen was reverted, `OPEN-255`).

| phase | what it added |
|---|---|
| C1 the workspace join | `view/glclient`, `view/cityview` (Bevy 0.19.1) as members outside `default-members`; `test.py view` (budget 7 min) |
| C2 the follow-cam | `citysim::view::snapshot`, `export-view`, `cityview --save --follow --shots`, `SimThread`, `shadow --save-at` |
| C3, C4 (TheCityGraphics) | the El on rail (4 lines, 20 stations, the duplicate tram cut); export `city-sim/0.2` (levels, cores, portals) |
| C5 the re-import | both maps from rail.json and the 0.2 export |
| C6a, C6b floors as tenancies | a tenancy is a `Building` `within` a host (`span`, `units`); 100k Lots 1,415 -> 8,000; floor rent (`Flow::FloorRent`), founding into units |
| C7 levels and lifts | `ExecState::Climb`, `World.lifts`, walkway portals; stairs at a person's pace |
| C8, C8b, C9b vertical gangs | Hideouts on floors, raids climb, lifts held and cut, the Spire 30-60 storeys, the landlord's answer ladder, nearest-Hideout raids |
| C9a far travel | the Farm deal, the body's own trip and fare, a move-in weighs both commutes; 100k legs over 120 min 33 -> 0 |
| the close's god fix | three god scenarios re-read against Y4 and X4; `household::camp_visit` sees every child of the parent held |

**Shadow V19** (`docs/SHADOW_V19.md`, `docs/shadow_v19/`): exit 1, 3, 5 met; 2 met with a gap (the raid's climb is the
event's line, not the member's body, `OPEN-264`); 4 partial (a climb and a lift ride on camera; the replay drifts
from the diary within hours, `OPEN-270`; riders drawn on a white slab, `OPEN-271`); **6 missed: 100k 11 ticks/s and
one 14.5 s tick at 21:00 every day** (`OPEN-265`). No minor harmed, `harm_blocked` 0. Housing cut the city's violence
(Murder 389 -> 272, evictions 7,242 -> 192) on a run that carried the 2k-calibrated table: re-read on the next 100k run.

## The close's tiers and report

Merge `[merge] RED (god); BUDGET OK 7m33s of 8m00s` (three god scenarios, fixed in `100c7cd`, merged `d41f927`);
milestone `[milestone] GREEN; BUDGET OK 11m36s of 20m00s`; lint 0 problems, 10 warnings (files over 1,500 lines).

| tier | last run | wall | budget | |
|---|---|---|---|---|
| change | main `d41f927` green | 3m37s | 4m00s | ok |
| merge | 20261101-151324 RED (fixed after) | 7m33s | 8m00s | ok |
| milestone | 20261101-152639 green | 11m36s | 20m00s | ok |
| view | C9a's branch green | 6m44s | 7m00s | ok |

God scenarios 20 of cap 20; asserted bullets 60 of 60; ignores 56. Every cap met.

## Decided (Dylan, this session)

- The planners' calls in three batches (in the spec's "Dylan's answers" 6-8): the El's routes, D2 offices at one per
  500 m^2 (a placeholder), the view at real time, the Spire's storeys, the landlord's answer, nearest-Hideout raids.
- **No street-crossing cost until there is a 3D world to look at** (traffic, lights): `OPEN-260`, the view milestone.
- **No Lua**: config hot-reload in the view instead (`OPEN-262`).

## Next, in order

1. **The view milestone.** First phase: config hot-reload (`OPEN-262`). Then a perf phase on the 21:00 tick and 100k
   ticks/s (`OPEN-265`), the camera's replay and drawing (`OPEN-270`, `OPEN-271`), the ring (`OPEN-149`), the pace
   tuned by eye, street crossing (`OPEN-260`), the cells (`OPEN-256`).
2. **Calibrate the stat table where bodies ride** (20k or the 100k island; `calibrate` reads the 2k map today), then
   one 100k run to re-read the El, Murder and violence (`OPEN-255`, `OPEN-254`).
3. Then M18 against the 3D client.

Open from this session: `OPEN-260` to `OPEN-272` (`OPEN-264` the raid's body, `265` perf, `266` the lift a constant,
`267` corp lives, `268` contradictions and loops, `269` the vertical city's leftovers, `270`/`271` the camera, `272` a Camp infant at the fence). Closed:
`OPEN-158`, `187`, `191`, `204`, `240`, `241`. Carried: `OPEN-221`, `254`, `255`, `256`, `257`, `252`, `253`, `258`,
`259`, `217`, `220`, `212`, `150`, `179`, `229`, `231`; standing `OPEN-062`, `039`, `057`, `059`, `077`, `069`, `070`.

## Process notes from this session

- **Calibrate where the mechanism lives.** The stat table regen ran on the 2k map, where nobody rides at the real pace;
  it zeroed every Statistical ride, killed the El at 100k and broke the change tier. Read a regenerated table's
  diff before committing it.
- **A close-only merge tier finds stale scenarios late** (two of three were Y4 and X4): cheap at the close ($7).
- **The follow-cam is not yet a proof tool**: without determinism, a replay from a save parts from the shadow's diary
  within hours. Record the shadow's frames rather than reintroduce a determinism gate (`OPEN-270`).
- C7 and C9a blew the 300-turn brief (501, 550 turns): split such phases at planning.

## Cost (API-rate $ equivalent, `tools/session_cost.py --last 1`, this session's part)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| C1 the workspace join | Opus | 278 | 149k | 11 |
| C3 the El on rail | Opus | 230 | 248k | 14 |
| C4 the export | Opus | 104 | 183k | 6 |
| C2 the follow-cam | Opus | 249 | 218k | 20 |
| C5 the re-import | Opus | 258 | 171k | 24 |
| C6a tenancies: the map and store | Opus | 345 | 293k | 39 |
| C6b tenancies: the economy | Opus | 442 | 297k | 50 |
| C7 levels and lifts | Opus | 501 | 361k | **62** |
| C8 the vertical gangs | Opus | 400 | 289k | 38 |
| C8b the Spire's storeys | Opus | 223 | 160k | 14 |
| C9b gang leftovers | Opus | 232 | 205k | 25 |
| C9a far travel | Opus | 550 | 271k | **73** |
| the close's god fix | Opus | 117 | 96k | 7 |
| Shadow V19 critics (8) and the watch | Sonnet | 655 | 103-119k | 22 |
| orchestrator (merges, briefs, runs, synthesis) | Opus | ~360 | 151k | ~30 |

The city milestone ≈ $435 for twelve phases, the exit read and the close. No agents or worktrees are left; the
runs are in `target/v19/` (the day-21 save, diaries, counters, tier prints).
