# Handoff: now

Rewritten at every session end; never appended. The session logs are `docs/history/SESSIONS.md` (the pre-close Now page of sessions 14-16 is its last entry). Last rewrite: **2026-10-19, the Lives at work close (sixteenth session).**

## Main

Main holds **Lives at work, closed** (`docs/LIVES_AT_WORK.md`, `SAVE_VERSION` 45): the service encounter and the venues (1), the household evening and visits (2), the civic counterparts (3), the street and freight, the pimp and the corner (4a/4b), the tower and the brokers (5), ages, careers and leaders (6), Stations and Spire homes at 20k (7), the gate (8a-8d, Shadow V5), and the `/code-review high` fix round's 11 findings (`b17a9fd`). § 15 "The close" has the final tiers and the reads. The milestone's nine env switches are retired (config paths kept; `OPEN-094` keeps the earlier milestones' ten). Earlier milestones: `docs/README.md`.

**All three tiers green on the final tree** (the close commit):

| tier | wall | budget | |
|---|---|---|---|
| change | 3m32s | 4m00s | ok |
| merge | 4m25s | 8m00s | ok (year 117 s, behaviour 60 s, god 52 + 18 + 18 s) |
| milestone | 13m50s | 20m00s | ok (`god_20k` 349 s; parity green but the FINDING rows `OPEN-038`, `OPEN-106`, `OPEN-107`) |

`test.py report --since 23a161d`: god 20/20 · asserted bullets 60/60 · 54 ignores (25 diag, 24 merge, 3 milestone, 2 gap) · 100 test files, 61,785 lines · `fn world()` copies outside `tests/common`: 30 · since `23a161d`: 44 files, +6,268 −650, new test files careers, civic, god_20k, household, service, stations, tower. Every cap met. Lint 0 problems, 6 warnings (test files over 1,500 lines: contracts, god, hitmarket, person, wages, `shadow.rs`; split as they are touched).

## Cost (sixteenth session, `python tools/session_cost.py --session cd4ee56c`, API-rate $ equivalent)

| phase | agent turns | median ctx | $ | helpers |
|---|---|---|---|---|
| 4a | 381 | 221k | 27 | murders + ripperdoc reads (Sonnet) 4 |
| 4b | 422 | 260k | 28 | |
| 5 | 409 | 304k | 30 | fix round 2 |
| 6 | 377 | 294k | 25 | fix round 3 |
| 7 | 422 | 288k | 37 | |
| 8a | 166 | 110k | 10 | regen fix 22, probes 4, rows 1, OPEN-103 3 |
| 8b | 473 | 317k | 39 | |
| 8c | 241 | 174k | 13 | six Sonnet diary readers 8, fix round 37 |
| OPEN-111 / 112 | 251 + 270 | 150-170k | 27 | follow-up 4 |
| 8d | 367 | 225k | 35 | fix round 6 |
| review | 115 | 287k | 10 | fix round 17 |
| orchestrator | 514 | 204k | 62 | |

Total $458 (1,249M cache read). The fifteenth session (phases 0-3) was ≈ $530 for half the phases, so the token rules cut the cost a phase by more than half (≈ $130 to ≈ $45). The fix rounds are a third of the coder spend: 8a's regen and 8c's Shadow fix are the big ones. Next lever: a phase's coder stays under ~250k median context (8a at 110k and 8c at 174k were the cheapest phases per row moved).

## In flight

Nothing. Every agent of the milestone is shut down, every worktree is removed and its branch deleted (`worktree-agent-aeb9f5f4cf9b9e4ee`, the parked phase 4 WIP `2c22096`, landed in 4a). Two empty dirs under `.claude/worktrees/` (`agent-a48242db0386a6fb5`, `agent-ab0f1e7a4f9d801b8`) were held open by this session's process: `rmdir` them after a restart.

**Incident at the close:** an idle agent answering the shutdown request ran `cd <its removed worktree> && …; rm -rf target` and wiped main's `target/` mid-tier (no tracked file lost; the `phase_measure.py` cache under `target/measure/` is gone and rebuilds on the next call). CLAUDE.md's cleanup rule now says absolute paths only.

## Next, in order

Shadow V6 (2026-10-20, `docs/SHADOW_V6.md`, critiques in `docs/shadow_v6/`) read the closed tree at 20k as the pre-view assessment. Dylan 2026-10-19: the game city is 20k and 2k is a test fixture (CLAUDE.md). **Proposed, not yet decided by Dylan:** the 3D view goes before the player character, and TheCityGraphics stays a separate repo until then (its Bevy client joins the workspace outside `default-members` when the view milestone starts; the assets stay out).

1. **The pre-view fix pass**: SHADOW_V6 § Before the camera rows 1-7 (`OPEN-137` the statue shift, `OPEN-138` the idle freeze, `OPEN-119`/`120`/`145` settlement by presence, `OPEN-139` the 20k legs, `OPEN-140` housing churn, `OPEN-141` parcels, `OPEN-142` the implant loop), with `OPEN-115`/`118`/`121`. Spec it as a milestone first.
2. **The island holds 20k** (`OPEN-136`): a finer import or more flats a lot; then move `core_20k`/`god_20k` to it.
3. **The view milestone** ("the city on screen": the Bevy client reads the sim, follow-cam on an NPC, no input), specced with `OPEN-149` (Statistical bodies on screen, the arrest jump) and `OPEN-150` (the cold start). Then M18 rewritten against that client; `OPEN-094` before its first phase.

## Open calls for Dylan

Rows of kind `decision` in `docs/OPEN.md`: new, **`OPEN-149`** (how the view draws Statistical bodies), **`OPEN-150`** (the cold start: pre-roll, seed gangs, or both), **`OPEN-135`** (the city seeds no children: 3 by day 10 on seed 42, so the School and the Camp read on a handful; seed households with children?). Standing: the harsh city and a pressure pass (`OPEN-062`), gang wars cooling (`OPEN-039`), faith equilibrium (`OPEN-057`), synth parity (`OPEN-059`), execs are cheap (`OPEN-077`), Ambient rent and the 2k flats (`OPEN-069`, `OPEN-070`).

## Standing decisions

`CLAUDE.md` § Standing decisions, each dated. The doctrine and the agent rules are there; `docs/TESTING.md` has the tiers, budgets and caps.
