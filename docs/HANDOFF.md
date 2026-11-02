# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md`. Last rewrite:
**2026-11-02, the thirty-first session: the view milestone (V1-V9, V8b) built and closed, Shadow V20.**

## Main

Main holds **the closed view milestone** (`docs/VIEW_MILESTONE.md`, each phase's "as built", "Implemented:
deviations" and "Closed (2026-11-02)"): the city watched live through an orbit follow-cam with a cutaway, people drawn
as TheCityGraphics' NPCs, riders in their cars, the ring Full around the camera, a shadow's day recorded and replayed,
lights and marked crossings, the El ridden, raids climbed in bodies, the cells' week. Change tier green on main after
V9 (`[change] GREEN; BUDGET OK 3m33s of 4m00s`); tests 1,472; `SAVE_VERSION` 126; `assets/stat_table.toml`
regenerated on the 20k island (V9).

| phase | what it added |
|---|---|
| V1 hot-reload | `cityview --config`, the R key, an mtime poll, `--reload-at`; `citysim::reload` |
| V2, V2b perf | the 21:00 Lounge-door stall (14.5 s -> 0.7 s), the Market pick, the shelf, Court; 100k tps 13.5 -> 18.9 |
| V3 people | NPC glbs by identity, walk/idle clips (`npc_clips.glb`), skinned within 40 m; indoor bodies hidden |
| V4 riders and climbs | El cars with riders, the lift cab, stairs, the body at its storey z, `--watch-vehicle` |
| V4b camera | an orbit perspective follow-cam (Q/E, Z/X, drag, wheel) and the cutaway (`[view] cut_*`); occbench retired |
| V5 the ring | `systems/ring.rs`: the camera's own Full/Coarse budget, promoted each tick from the schedule |
| V6 the replay | `shadow --record`, `--record-view`, `--from-save`; `cityview --sim` on the diary's minute |
| V7 crossings | the export's carriageways and zebras, signals per junction, walkers wait for the green man; `lights.rs` |
| V8 cells and raids | the raid column as bodies; `[cells]`: buy-outs, kin bail, early release, a work detail, cards |
| V8b the muster | a raid's muster holds for a crew of three or breaks up naming who came |
| V9 the El | stations, not bends; a minute on foot weighs two aboard; the stat table on the 20k island |
| V10 after the close | an indoor body on its storey's massing box (the cutaway finds its floor); the light follows the sim's hour, `[view] light_*` |

**Shadow V20** (`docs/SHADOW_V20.md`, `docs/shadow_v20/`): exits 2, 3, 4, 6 met; 1 partial (the 00:11 hire 2-4.2 s,
`OPEN-265`); 5: the reload met, the pace by eye open (`OPEN-283`, Dylan's). No minor harmed. The cells' early release
halved the jailed and violence rose 525 -> 807 (a finding, `OPEN-256`). The work and the gangs still read thin.

## The close's tiers and report

Merge `[merge] GREEN; BUDGET BREACHED 9m04s of 8m00s` (the critics shared the box); milestone `[milestone] GREEN;
BUDGET OK 9m06s of 20m00s`; view `[view] GREEN; BUDGET OK 6m00s of 7m00s`; lint 0 problems, 10 warnings.

| tier | last run | wall | budget | |
|---|---|---|---|---|
| change | 20261102-024432 green | 3m33s | 4m00s | ok |
| merge | 20261102-083928 green | 9m04s | 8m00s | **BREACHED** (box load) |
| milestone | 20261102-085529 green | 9m06s | 20m00s | ok |
| view | 20261102-090451 green | 6m00s | 7m00s | ok |

God scenarios 20 of cap 20; asserted bullets 60 of 60; ignores 56; no test files added since `0f54230`. Every cap met.

## Decided (Dylan, this session)

- **The orbit camera and the cutaway** replace locked iso and the portal (CLAUDE.md standing decisions).
- **Lights and marked crossings** for street crossing; **no filler traffic**: vehicles on screen are citizens' and
  self-driving sim bodies, added later (`OPEN-275`).

## Next, in order

1. **Dylan: the pace by eye** (`OPEN-283`), now in daylight and a readable night (V10): `cityview --save target/v20/isle100k_s42_d21/saves/42-30240.ron --follow
   24590` (Nerys Underhill, office worker, floor 27), edit `assets/config.toml`, it reloads live.
2. **A short leftovers pass**, one agent each: `OPEN-282` (a starving line prints age and index: the child rule must be
   checkable), `OPEN-280` (the guard paid for a half shift), `OPEN-281` (retired at 62), `OPEN-265` (the 00:11 hire),
   `OPEN-278` (raids on empty Hideouts), `OPEN-287` (the jail drawn as one 3,000-body crowd), `OPEN-285` (indoor
   bodies not room-aware, sleepers standing).
3. **M18 rewritten against the 3D client** (the roadmap's order); real traffic (`OPEN-275`) placed by Dylan; chrome on
   the company's credit and its repossession (`OPEN-286`, Dylan's idea); the real lighting pass (`OPEN-284`).

Open from this session: `OPEN-273` to `OPEN-287` (less `276`, `277` closed). Closed: `OPEN-149`, `255`, `262`, `264`,
`270`, `271`, `276`, `277`. Carried: `OPEN-256`, `260`, `265`, `217`, `253`, `254`, `221`, `257`, `252`, `258`, `259`,
`266` to `269`, `272`; standing `OPEN-062`, `039`, `057`, `059`, `077`, `069`, `070`.

## Process notes from this session

- **Measure on main, not on the coder's base.** V8 reported "no raid reaches a door" from its pre-V7 branch and V8b
  found it false on main; V8's 736 starvation deaths at 20k did not reproduce either. A phase's alarming number gets
  re-read on main before it becomes a phase.
- **Two phases re-pinning fixtures on separate branches** (V8, V9) merged clean, but one change-tier run on main after
  them was the cheap check (green, 3m33s).

## Cost (API-rate $ equivalent, `tools/session_cost.py`)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| V1 hot-reload | Opus | 170 | 173k | 21 |
| V2 the 21:00 tick | Opus | 182 | 140k | 27 |
| V3 people | Opus | 337 | 232k | 31 |
| V4 riders and climbs | Opus | 244 | 202k | 24 |
| V4b the camera and cutaway | Opus | 305 | 287k | 28 |
| V4/V4b merge round | Sonnet | 36 | 68k | 1 |
| V5 the ring | Opus | 232 | 211k | 22 |
| V6 the replay | Opus | 211 | 217k | 27 |
| V7 crossings | Opus | 387 | 295k | 42 |
| V2b steady ticks | Opus | 297 | 179k | 42 |
| V8 cells and raid bodies | Opus | 449 | 263k | **53** |
| V9 the El | Opus | 319 | 203k | 35 |
| V8b the muster | Opus | 279 | 190k | 30 |
| Shadow V20 critics (3) | Sonnet | 121 | 80-96k | 4 |
| V10 the cutaway's floor and the light | Opus | 184 | 160k | 12 |
| orchestrator (merges, briefs, the close) | Opus | 516 | 156k | 32 |

The view milestone ≈ $432 for fourteen phases, the exit read and the close. No coders or worktrees are left (one
untracked leftover folder, `.claude/worktrees/agent-a2d06b9d9985759f3`, holds a locked file: delete by hand); the runs
are in `target/v20/`.
