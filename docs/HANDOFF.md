# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md` (the sixteenth session's close page is its last entry). Last rewrite: **2026-10-20, the seventeenth session: the assessment, Shadow V6 and the new order.**

## Main

Main holds **Lives at work, closed** (`docs/LIVES_AT_WORK.md`, `SAVE_VERSION` 45; all three tiers green at `a25fdc5`). This session added no sim change, only these:

- the V6 shadow tool, merged as `5d77afa`, change only in `citysim-cli/src/shadow.rs`. It adds the camera block, labels each served line with its kind and side, adds `[parcel]` lines and `--map`. The off identity on seed 42 × 30 is identical; change GREEN (8m14s under load, budget breached), merge GREEN 5m09s;
- **Shadow V6** (`docs/SHADOW_V6.md`, critiques in `docs/shadow_v6/`, the runs in `docs/plans/shadow_v6_runs.md`);
- `OPEN-136..150`;
- roadmap addendum 20;
- five standing decisions in CLAUDE.md.

## What the session decided (Dylan, 2026-10-19/20; all in CLAUDE.md § Standing decisions)

- **The game city is 20k; 2k is a test fixture.** Reads, calibration and acceptance are judged at 20k. A phase reports 20k seed 42 × 30 beside 2k. `OPEN-136` asks for a 2k fixture cut from the island.
- **M18 waits.** The order:
  1. the pre-view fix pass;
  2. the island holding 20k;
  3. the view milestone;
  4. M18, rewritten against the 3D client. Its spec predates M13-M17 as built, and it moves its camera, HUD and input off the macroquad debug app.
- **Nothing in camera range is Statistical** (`OPEN-149`). The ring:
  - Full in view, a Coarse ring around it, Statistical beyond.
  - It needs three changes: rank by camera distance ahead of story class, re-assign on camera moves (today the assignment is hourly), and on Statistical → Coarse place the body where its schedule says it is (today it keeps a stale demotion snapshot).
  - Cost probe: about 1.3 µs per Full agent per tick at 20k (50 / 500 / 2,000 Full = 342 / 284 / 186 tps on day 6). Two thousand Full at 10 ticks/s is about 5 % of a core. The sim cost is not the limit; rendering is.
- **The player never starts on day 0** (`OPEN-150`, which folds in `OPEN-135`). Build the start from a starting-condition seed (preferred over a pre-run). The seed must be consistent with itself (turf with members, debts with purses, edges with memories), or week 1 becomes the sim correcting it. A cheap way to get one: generate it once from a pre-run, then curate and freeze it.
- **TheCityGraphics stays a separate repo** until the view milestone. Then its Bevy client (`engine/glclient`, `engine/cityview`) joins this workspace outside `default-members`, with its own test tier. The Python/Blender pipeline and the roughly 1.1 GB of PNGs stay in TheCityGraphics and are read as a generated-assets path, the same way `import-graphics` reads them today. The reasons: no LFS, Bevy 0.19 with a vendored patched `bevy_pbr` would wreck the change tier's budget, and every worktree would check out about 1 GB.

## Shadow V6 in five lines

1. The people are alive. Their trades mostly are not: every `Role::Trade` shift is one 530-540 min `ClerkWork` on one tile (55 % of labour pick-days), and the sweeper stands for 530 min.
2. Encounters settle by link, not presence. 76 % of named counterparts are Statistical. The super calls on tenants who are at work or asleep, venue customers settle in a 21:00 batch, and the arms sale books at midnight.
3. V5's dead trades are still dead: the cab, the courier (0 parcels in 36 rounds), the bouncer, the joytoy, the leader and the scav. The crime trades lost their on-screen beats.
4. **The island does not hold 20k.** About 8,000 people are homeless from d0, employment is 22-29 % and 231 are starving by d29. The import has 93 Blocks per 1,000 people against the generator's 200.
5. Day 0 is cold: 19 of 48 archetypes are absent, there are no gangs, and week 1 is a backlog.

## In flight

Nothing. Every agent of the session has finished, and the V6 coder's worktree is removed (its `target/`, with the diaries, deleted by absolute path; the commands to regenerate them are in `docs/plans/shadow_v6_runs.md`). The two empty dirs from the sixteenth session under `.claude/worktrees/` may still need an `rmdir` after a restart.

TheCityGraphics (separate repo, its own sessions) sits at its checkpoint T on the unmerged branch `session33-city-p9`. Its handoff has three questions waiting for Dylan: the look of the kit'd towers, all floors playable, and the overnight D4 regen.

## Next, in order (two tracks in parallel; Dylan 2026-10-20)

**Track A, here: the pre-view fix pass.** Spec it as a milestone first, with Dylan. The design question is what a visible shift looks like per trade (`OPEN-137`): legs, rounds, crates and fares built from existing actions where possible. Scope is SHADOW_V6 § Before the camera, rows 1-7:

| row | OPEN ids |
|---|---|
| the statue shift | `OPEN-137` |
| the idle freeze | `OPEN-138` |
| settlement by presence | `OPEN-119`, `OPEN-120`, `OPEN-145` |
| the 20k legs | `OPEN-139` |
| housing churn | `OPEN-140` |
| parcels | `OPEN-141` |
| the implant loop | `OPEN-142` |

Add the dead trades `OPEN-115` (cab), `OPEN-118`/`OPEN-128` (leaders) and `OPEN-121` (bouncer). Retire `OPEN-094`'s ten env switches inside it. The gate is a Shadow V7 at 20k with the V6 tool.

**Track B: the island holds 20k** (`OPEN-136`): a finer import (k < 4) or more flats a lot, then a 30-day 20k run that houses and employs like `gen:20000`. After that, move `core_20k`/`god_20k` to the island and cut the 2k fixture from it. This can run as its own agent here (the importer is `citysim-cli/src/import_graphics.rs`, `docs/IMPORT_GRAPHICS.md`) or on the graphics side. TheCityGraphics stays read-only from here.

**Then the view milestone ("the city on screen")**: the Bevy client reads the sim, with a follow-cam on an NPC and no input. Its spec carries `OPEN-149` (the ring), the sentencing jump (16-608 tiles; a "taken away" beat or an escort walk), the weekly venue rest day, `OPEN-150` (the starting seed) and the snapshot contract (sim tile ↔ world metres, building and floor ↔ interior spot). Then M18.

## Open calls for Dylan

- The fix pass's visible shift per trade (`OPEN-137`): to be specced together.
- The starting-condition seed's shape (`OPEN-150`): authored, generated from a pre-run and frozen, or both. It also decides whether the player gets world options.
- Standing: the harsh city and a pressure pass (`OPEN-062`), gang wars cooling (`OPEN-039`), faith equilibrium (`OPEN-057`), synth parity (`OPEN-059`), execs are cheap (`OPEN-077`), Ambient rent and the 2k flats (`OPEN-069`, `OPEN-070`).
- TheCityGraphics' checkpoint T (in that repo's HANDOFF).

## Cost (seventeenth session, API-rate $ equivalent)

| part | agent | turns | median ctx | $ |
|---|---|---|---|---|
| the graphics survey | Sonnet | 23 | 40k | 0.4 |
| V6 phases 0-1 (tool + windows) | Opus | 213 | 203k | 20 |
| V6 critics, seven in parallel (a Workflow) | Sonnet | 96-126 each | 148-172k | ≈ 30 |
| orchestrator | Opus | ≈ 130 | 132k | ≈ 10 |

Total ≈ $60. V5's read cost about $45 plus a $37 fix round. `tools/session_cost.py --session` misses Workflow subagents (their transcripts are under `<session>/subagents/workflows/<run>/`; cost them with `--dir <that dir> --last 10`). Teach the tool to walk that folder (`OPEN-151`).

## Standing decisions

`CLAUDE.md` § Standing decisions, each dated. The doctrine and the agent rules are there; `docs/TESTING.md` has the tiers, budgets and caps.
